use crate::models::{
    AutomationCandidateV1, CheckInV1, DiagnosticV1, MetricValue, SettingsV1, TimelineBlockV1,
};
use chrono::Utc;
use std::collections::HashMap;

pub fn create_diagnostic(
    blocks: &[TimelineBlockV1],
    checkins: &[CheckInV1],
    days_observed: f64,
    settings: &SettingsV1,
) -> DiagnosticV1 {
    let active_seconds: i64 = blocks
        .iter()
        .filter(|block| !block.afk)
        .map(|block| block.duration_seconds)
        .sum();
    let focus_seconds: i64 = blocks
        .iter()
        .filter(|block| block.category == "focus")
        .map(|block| block.duration_seconds)
        .sum();
    let switches = blocks
        .windows(2)
        .filter(|pair| pair[0].app != pair[1].app)
        .count();
    let answered = checkins
        .iter()
        .filter(|item| item.status == "answered")
        .count();
    let candidates = score_candidates(checkins, days_observed);
    let mut recommendations = Vec::new();
    if active_seconds > 0 && focus_seconds as f64 / (active_seconds as f64) < 0.35 {
        recommendations
            .push("Reserva un bloque protegido de 45 minutos antes de abrir comunicación.".into());
    }
    if switches as f64 > (active_seconds as f64 / 3600.0 * 12.0).max(8.0) {
        recommendations
            .push("Agrupa mensajes y tareas administrativas en dos ventanas del día.".into());
    }
    if checkins.iter().any(|item| item.status == "pending") {
        recommendations.push(
            "Resuelve los intervalos pendientes para mejorar la calidad del diagnóstico.".into(),
        );
    }
    if let Some(candidate) = candidates.first() {
        recommendations.push(format!(
            "Valida “{}” como primer experimento de automatización.",
            candidate.activity_label
        ));
    }
    if recommendations.is_empty() {
        recommendations.push("Sigue registrando contexto; el patrón actual todavía es estable y no exige un cambio inmediato.".into());
    }
    let generated_at = Utc::now().to_rfc3339();
    DiagnosticV1 {
        schema_version: 1,
        range_start: blocks
            .first()
            .map(|item| item.start.clone())
            .unwrap_or_else(|| generated_at.clone()),
        range_end: blocks
            .last()
            .map(|item| item.end.clone())
            .unwrap_or_else(|| generated_at.clone()),
        days_observed,
        metrics: vec![
            MetricValue {
                label: "Tiempo activo".into(),
                value: active_seconds as f64,
                unit: "seconds".into(),
                delta: None,
            },
            MetricValue {
                label: "Trabajo enfocado".into(),
                value: if active_seconds > 0 {
                    (focus_seconds as f64 / active_seconds as f64 * 100.0).round()
                } else {
                    0.0
                },
                unit: "percent".into(),
                delta: None,
            },
            MetricValue {
                label: "Cambios de contexto".into(),
                value: switches as f64,
                unit: "count".into(),
                delta: None,
            },
            MetricValue {
                label: "Check-ins respondidos".into(),
                value: answered as f64,
                unit: "count".into(),
                delta: None,
            },
        ],
        recommendations: recommendations.into_iter().take(3).collect(),
        prompt: build_prompt(
            candidates.first(),
            &settings.role,
            &settings.goals,
            days_observed,
        ),
        candidates,
        preliminary: days_observed < 7.0,
        generated_at,
    }
}

fn score_candidates(checkins: &[CheckInV1], days_observed: f64) -> Vec<AutomationCandidateV1> {
    let mut groups: HashMap<String, Vec<&CheckInV1>> = HashMap::new();
    for item in checkins
        .iter()
        .filter(|item| item.status == "answered" && item.repetitive == Some(true))
    {
        let Some(label) = item
            .activity_label
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        groups.entry(label.to_lowercase()).or_default().push(item);
    }
    let mut candidates: Vec<_> = groups
        .into_iter()
        .filter(|(_, items)| items.len() >= 2)
        .map(|(key, items)| {
            let weekly_frequency = round(items.len() as f64 / days_observed.max(1.0) * 5.0, 1);
            let observed_hours: f64 = items
                .iter()
                .filter_map(|item| {
                    let start = crate::models::parse_utc(&item.interval_start).ok()?;
                    let end = crate::models::parse_utc(&item.interval_end).ok()?;
                    Some((end - start).num_seconds().max(0) as f64 / 3600.0)
                })
                .sum();
            let estimated_weekly_hours = round(observed_hours / days_observed.max(1.0) * 5.0, 1);
            let impact_score = (estimated_weekly_hours * 5.0 + weekly_frequency * 1.5)
                .round()
                .min(40.0) as i32;
            let clear_ratio = items
                .iter()
                .filter(|item| item.rules_clear == Some(true))
                .count() as f64
                / items.len() as f64;
            let feasibility_score = (18.0 + clear_ratio * 17.0).round() as i32;
            let risk_score = if clear_ratio > 0.6 { 6 } else { 14 };
            AutomationCandidateV1 {
                id: format!("candidate-{}", slug(&key)),
                activity_label: items[0].activity_label.clone().unwrap_or(key),
                estimated_weekly_hours,
                weekly_frequency,
                impact_score,
                feasibility_score,
                risk_score,
                priority_score: (impact_score + feasibility_score - risk_score).clamp(0, 100),
                evidence: vec![
                    format!("{} registros durante el periodo", items.len()),
                    format!("{weekly_frequency} repeticiones estimadas por semana"),
                    if clear_ratio > 0.6 {
                        "La mayoría sigue reglas claras"
                    } else {
                        "Necesita aclarar reglas antes de automatizar"
                    }
                    .into(),
                ],
            }
        })
        .collect();
    candidates.sort_by(|left, right| right.priority_score.cmp(&left.priority_score));
    candidates
}

fn build_prompt(
    candidate: Option<&AutomationCandidateV1>,
    role: &str,
    goals: &[String],
    days_observed: f64,
) -> String {
    let role = if role.trim().is_empty() {
        "profesional independiente"
    } else {
        role.trim()
    };
    let goals = if goals.is_empty() {
        "recuperar tiempo para trabajo de mayor valor".into()
    } else {
        goals.join(", ")
    };
    let Some(candidate) = candidate else {
        return format!("Actúa como consultor de automatización. Todavía no hay suficiente evidencia para elegir una tarea. Ayúdame a diseñar un estudio de trabajo de {days_observed:.0} días para identificar procesos repetitivos, medibles y de bajo riesgo. Mi rol es: {role}. Mis objetivos son: {goals}.");
    };
    format!(
        "Actúa como arquitecto de automatización pragmático. Quiero decidir qué automatizar primero basándome en evidencia de mi trabajo, no en tendencias.\n\nCONTEXTO\n- Rol: {role}\n- Objetivos: {goals}\n- Días observados: {days_observed:.0}\n\nCANDIDATO PRINCIPAL\n- Actividad: {}\n- Frecuencia estimada: {} veces por semana\n- Tiempo estimado: {} horas por semana\n- Impacto: {}/40\n- Facilidad: {}/35\n- Riesgo: {}/25\n- Evidencia: {}\n\nENTREGABLE\n1. Diagnostica si realmente conviene automatizar esta actividad primero.\n2. Separa pasos deterministas de pasos que requieren criterio humano.\n3. Propón un MVP que pueda validar en menos de una semana.\n4. Recomienda herramientas concretas y una arquitectura simple.\n5. Define riesgos, controles y una métrica de éxito en horas ahorradas.\n6. Si no es el mejor primer candidato, explica qué información falta antes de descartarlo.\n\nNo inventes datos. Señala cada supuesto y prioriza una automatización reversible.",
        candidate.activity_label,
        candidate.weekly_frequency,
        candidate.estimated_weekly_hours,
        candidate.impact_score,
        candidate.feasibility_score,
        candidate.risk_score,
        candidate.evidence.join("; ")
    )
}

fn slug(value: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            separator = false;
        } else if !separator && !slug.is_empty() {
            slug.push('-');
            separator = true;
        }
    }
    slug.trim_end_matches('-').to_string()
}

fn round(value: f64, digits: i32) -> f64 {
    let factor = 10_f64.powi(digits);
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::iso_now;

    fn answered(id: &str) -> CheckInV1 {
        CheckInV1 {
            id: id.into(),
            interval_start: iso_now(),
            interval_end: iso_now(),
            reason: "ambiguous".into(),
            status: "answered".into(),
            activity_label: Some("Preparar reporte".into()),
            category: Some("admin".into()),
            project: None,
            repetitive: Some(true),
            rules_clear: Some(true),
            created_at: iso_now(),
            answered_at: Some(iso_now()),
        }
    }

    #[test]
    fn needs_repeated_evidence_for_candidate() {
        assert!(score_candidates(&[answered("1")], 7.0).is_empty());
        let candidates = score_candidates(&[answered("1"), answered("2")], 7.0);
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].priority_score > 0);
    }
}
