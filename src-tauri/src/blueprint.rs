use crate::models::{iso_now, parse_utc, WorkflowIntakeV1};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const BLUEPRINT_SCHEMA_VERSION: u8 = 1;
pub const BLUEPRINT_EXPORT_SCHEMA_VERSION: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StepDispositionV1 {
    Unassessed,
    Automatable,
    HumanRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DataSensitivityV1 {
    Unassessed,
    Low,
    Moderate,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SideEffectLevelV1 {
    Unassessed,
    None,
    Reversible,
    Irreversible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationModeV1 {
    Unassessed,
    None,
    Sample,
    EveryRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepBoundaryV1 {
    pub step_id: String,
    pub disposition: StepDispositionV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationBlueprintDraftV1 {
    pub schema_version: u8,
    pub id: String,
    pub candidate_id: String,
    pub workflow_intake_id: String,
    pub source_intake_updated_at: String,
    pub revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runs_per_week: Option<u16>,
    pub step_boundaries: Vec<StepBoundaryV1>,
    pub data_sensitivity: DataSensitivityV1,
    pub side_effect_level: SideEffectLevelV1,
    pub verification_mode: VerificationModeV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_minutes_per_run: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_plan: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveAutomationBlueprintInputV1 {
    pub draft: AutomationBlueprintDraftV1,
    #[serde(default)]
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintSourceV1 {
    pub trigger: String,
    pub inputs: String,
    pub desired_output: String,
    pub tools: Vec<String>,
    pub steps: Vec<BlueprintSourceStepV1>,
    pub human_decisions: String,
    pub frequency: String,
    pub minutes_per_run: u16,
    pub pain_points: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintSourceStepV1 {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintEvidenceV1 {
    pub observed: Vec<String>,
    pub declared: Vec<String>,
    pub repeated_observations: u32,
    pub observed_days: f64,
    pub rules_quality: f64,
    pub observed_weekly_frequency: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintCompletenessV1 {
    pub score: u8,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreComponentV1 {
    pub key: &'static str,
    pub points: u8,
    pub maximum: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreV1 {
    pub score: u8,
    pub components: Vec<ScoreComponentV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintEstimatesV1 {
    pub weekly_baseline_minutes: u32,
    pub weekly_residual_minutes: u32,
    pub weekly_savings_minutes: u32,
    pub effort: u8,
    pub operational_risk: u8,
    pub confidence: u8,
    pub components: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlueprintStatusV1 {
    Draft,
    Ready,
    Stale,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationBlueprintV1 {
    pub schema_version: u8,
    pub draft: AutomationBlueprintDraftV1,
    pub source: BlueprintSourceV1,
    pub evidence: BlueprintEvidenceV1,
    pub status: BlueprintStatusV1,
    pub completeness: BlueprintCompletenessV1,
    pub estimates: BlueprintEstimatesV1,
    pub rendered_markdown: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<BlueprintConflictV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintConflictV1 {
    pub current_revision: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationBlueprintExportEnvelopeV2 {
    pub schema_version: u8,
    pub export_kind: &'static str,
    pub exported_at: String,
    pub blueprint: AutomationBlueprintV1,
    pub deletion_boundary: String,
}

pub fn source_step_id(position: usize) -> String {
    format!("step-{:02}", position + 1)
}

pub fn validate_blueprint_locale(locale: &str) -> Result<(), String> {
    if matches!(locale, "es-419" | "en") {
        Ok(())
    } else {
        Err("El idioma del blueprint no es compatible".into())
    }
}

pub fn ephemeral_draft(intake: &WorkflowIntakeV1) -> AutomationBlueprintDraftV1 {
    let now = iso_now();
    AutomationBlueprintDraftV1 {
        schema_version: BLUEPRINT_SCHEMA_VERSION,
        id: format!("blueprint-{}", intake.candidate_id),
        candidate_id: intake.candidate_id.clone(),
        workflow_intake_id: intake.id.clone(),
        source_intake_updated_at: intake.updated_at.clone(),
        revision: 0,
        runs_per_week: None,
        step_boundaries: intake
            .steps
            .iter()
            .enumerate()
            .map(|(position, _)| StepBoundaryV1 {
                step_id: source_step_id(position),
                disposition: StepDispositionV1::Unassessed,
                rationale: None,
            })
            .collect(),
        data_sensitivity: DataSensitivityV1::Unassessed,
        side_effect_level: SideEffectLevelV1::Unassessed,
        verification_mode: VerificationModeV1::Unassessed,
        verification_minutes_per_run: None,
        recovery_plan: None,
        created_at: now.clone(),
        updated_at: now,
    }
}

pub fn draft_from_input(
    intake: &WorkflowIntakeV1,
    existing: Option<&AutomationBlueprintDraftV1>,
    input: AutomationBlueprintDraftV1,
) -> Result<AutomationBlueprintDraftV1, String> {
    if input.schema_version != BLUEPRINT_SCHEMA_VERSION {
        return Err("La versión del blueprint no es compatible".into());
    }
    if input.candidate_id != intake.candidate_id {
        return Err("El candidato no coincide con el intake fuente".into());
    }
    let expected_id = format!("blueprint-{}", intake.candidate_id);
    if input.id != expected_id
        || input.workflow_intake_id != intake.id
        || input.source_intake_updated_at != intake.updated_at
    {
        return Err("Los campos fuente del blueprint no pueden modificarse".into());
    }
    let mut draft = existing.cloned().unwrap_or_else(|| ephemeral_draft(intake));
    let now = iso_now();
    draft.source_intake_updated_at = intake.updated_at.clone();
    draft.workflow_intake_id = intake.id.clone();
    draft.runs_per_week = input.runs_per_week;
    draft.step_boundaries = input
        .step_boundaries
        .into_iter()
        .map(|mut boundary| {
            boundary.rationale = boundary
                .rationale
                .map(|rationale| normalize_text(&rationale));
            boundary
        })
        .collect();
    draft.data_sensitivity = input.data_sensitivity;
    draft.side_effect_level = input.side_effect_level;
    draft.verification_mode = input.verification_mode;
    draft.verification_minutes_per_run = input.verification_minutes_per_run;
    draft.recovery_plan = input.recovery_plan.map(|value| normalize_text(&value));
    draft.updated_at = now;
    validate_draft(&draft, intake)?;
    Ok(draft)
}

pub fn refresh_draft(
    mut draft: AutomationBlueprintDraftV1,
    intake: &WorkflowIntakeV1,
) -> Result<AutomationBlueprintDraftV1, String> {
    draft.workflow_intake_id = intake.id.clone();
    draft.source_intake_updated_at = intake.updated_at.clone();
    draft.step_boundaries = ephemeral_draft(intake).step_boundaries;
    draft.updated_at = iso_now();
    validate_draft(&draft, intake)?;
    Ok(draft)
}

pub fn validate_draft(
    draft: &AutomationBlueprintDraftV1,
    intake: &WorkflowIntakeV1,
) -> Result<(), String> {
    if draft.schema_version != BLUEPRINT_SCHEMA_VERSION {
        return Err("La versión del blueprint no es compatible".into());
    }
    validate_identifier(&draft.id, 160)?;
    validate_candidate_id(&draft.candidate_id)?;
    validate_candidate_id(&draft.workflow_intake_id)?;
    if draft.candidate_id != intake.candidate_id || draft.workflow_intake_id != intake.id {
        return Err("El blueprint no coincide con su intake fuente".into());
    }
    parse_utc(&draft.source_intake_updated_at)?;
    let created_at = parse_utc(&draft.created_at)?;
    let updated_at = parse_utc(&draft.updated_at)?;
    if updated_at < created_at {
        return Err("updatedAt no puede ser anterior a createdAt".into());
    }
    if created_at > chrono::Utc::now() + chrono::Duration::minutes(5)
        || updated_at > chrono::Utc::now() + chrono::Duration::minutes(5)
    {
        return Err("Las fechas del blueprint no pueden estar en el futuro".into());
    }
    if let Some(runs) = draft.runs_per_week {
        if !(1..=1_000).contains(&runs) {
            return Err("Las ejecuciones por semana deben estar entre 1 y 1000".into());
        }
    }
    if let Some(minutes) = draft.verification_minutes_per_run {
        if minutes > 1_440 {
            return Err("La verificación debe estar entre 0 y 1440 minutos".into());
        }
    }
    if draft.step_boundaries.len() != intake.steps.len() {
        return Err("Debe clasificar exactamente los pasos del intake".into());
    }
    for (position, boundary) in draft.step_boundaries.iter().enumerate() {
        if boundary.step_id != source_step_id(position) {
            return Err("Los límites de pasos no coinciden con el intake".into());
        }
        if let Some(rationale) = &boundary.rationale {
            validate_optional_text(rationale, 1_000, "La justificación del paso")?;
        }
        if boundary.disposition == StepDispositionV1::Unassessed && boundary.rationale.is_some() {
            return Err("Un paso sin clasificar no puede tener justificación".into());
        }
    }
    if let Some(plan) = &draft.recovery_plan {
        validate_optional_text(plan, 4_000, "El plan de recuperación")?;
    }
    let encoded = serde_json::to_vec(draft).map_err(|error| error.to_string())?;
    if encoded.len() > 40_000 {
        return Err("El blueprint excede el tamaño local permitido".into());
    }
    Ok(())
}

fn validate_candidate_id(value: &str) -> Result<(), String> {
    validate_identifier(value, 128)
}

fn validate_identifier(value: &str, maximum: usize) -> Result<(), String> {
    if value.is_empty()
        || value.len() > maximum
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(format!(
            "El identificador debe usar entre 1 y {maximum} caracteres ASCII seguros"
        ));
    }
    Ok(())
}

fn validate_optional_text(value: &str, maximum: usize, label: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.chars().count() > maximum
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{label} debe tener entre 1 y {maximum} caracteres sin controles"
        ));
    }
    Ok(())
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn assess_blueprint(
    draft: AutomationBlueprintDraftV1,
    intake: &WorkflowIntakeV1,
    total_observations: u32,
    repeated_observations: u32,
    observed_days: f64,
    rules_quality: f64,
    observed_weekly_frequency: f64,
    locale: &str,
) -> Result<AutomationBlueprintV1, String> {
    validate_blueprint_locale(locale)?;
    validate_draft(&draft, intake)?;
    let completeness_score = completeness(&draft, intake);
    let completeness = BlueprintCompletenessV1 {
        score: completeness_score.score,
        missing: missing_fields(&draft, intake),
    };
    let stale = draft.source_intake_updated_at != intake.updated_at;
    let has_automatable = draft
        .step_boundaries
        .iter()
        .any(|step| step.disposition == StepDispositionV1::Automatable);
    // `repeated_observations` comes only from check-ins that the user explicitly
    // marked as repetitive. Recorded intervals alone never open the READY gate.
    let user_confirmed_repetitions = repeated_observations;
    let status = if stale {
        BlueprintStatusV1::Stale
    } else if completeness.score == 100 && user_confirmed_repetitions >= 2 && has_automatable {
        BlueprintStatusV1::Ready
    } else if intake.steps.is_empty() {
        BlueprintStatusV1::Blocked
    } else {
        BlueprintStatusV1::Draft
    };
    let source = BlueprintSourceV1 {
        trigger: intake.trigger.clone(),
        inputs: intake.inputs.clone(),
        desired_output: intake.desired_output.clone(),
        tools: intake.tools.clone(),
        steps: intake
            .steps
            .iter()
            .enumerate()
            .map(|(position, label)| BlueprintSourceStepV1 {
                id: source_step_id(position),
                label: label.clone(),
            })
            .collect(),
        human_decisions: intake.human_decisions.clone(),
        frequency: intake.frequency.clone(),
        minutes_per_run: intake.minutes_per_run,
        pain_points: intake.pain_points.clone(),
    };
    let observed = if total_observations == 0 {
        Vec::new()
    } else if locale == "en" {
        vec![
            format!("{total_observations} answered check-in interval(s) were recorded locally"),
            format!("The selected recorded intervals cover {observed_days} distinct UTC day(s)"),
        ]
    } else {
        vec![
            format!("Se registraron localmente {total_observations} intervalo(s) de check-in respondido(s)"),
            format!("Los intervalos registrados seleccionados cubren {observed_days} día(s) UTC distinto(s)"),
        ]
    };
    let rules_percentage = (rules_quality.clamp(0.0, 1.0) * 100.0).round();
    let declared = if locale == "en" {
        vec![
            "Workflow intake and activity-label association supplied by you".into(),
            format!("You marked {user_confirmed_repetitions} linked check-in(s) as repetitive"),
            format!("Your clear-rules confirmations yield {rules_percentage}% rules quality"),
        ]
    } else {
        vec![
            "Intake del workflow y asociación por etiqueta de actividad declarados por ti".into(),
            format!("Marcaste {user_confirmed_repetitions} check-in(s) vinculado(s) como repetitivo(s)"),
            format!("Tus confirmaciones de reglas claras producen {rules_percentage}% de calidad de reglas"),
        ]
    };
    let evidence = BlueprintEvidenceV1 {
        observed,
        declared,
        repeated_observations,
        observed_days,
        rules_quality,
        observed_weekly_frequency,
    };
    let estimates = estimates(&draft, intake, &evidence);
    let assumptions = assumptions(&draft, locale);
    let rendered_markdown = render(
        &draft,
        &source,
        &intake.candidate_label,
        &evidence,
        status,
        &estimates,
        &assumptions,
        locale,
    );
    Ok(AutomationBlueprintV1 {
        schema_version: BLUEPRINT_SCHEMA_VERSION,
        draft,
        source,
        evidence,
        status,
        completeness,
        estimates,
        rendered_markdown,
        conflict: None,
    })
}

pub fn completeness(draft: &AutomationBlueprintDraftV1, intake: &WorkflowIntakeV1) -> ScoreV1 {
    let source_fields = [
        !intake.trigger.is_empty(),
        !intake.inputs.is_empty(),
        !intake.desired_output.is_empty(),
        !intake.tools.is_empty(),
        !intake.steps.is_empty(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count() as u8
        * 6;
    let classified = draft
        .step_boundaries
        .iter()
        .filter(|boundary| {
            boundary.disposition != StepDispositionV1::Unassessed
                && boundary
                    .rationale
                    .as_ref()
                    .is_some_and(|value| !value.trim().is_empty())
        })
        .count();
    let boundary_points = if intake.steps.is_empty() {
        0
    } else {
        ((classified as f64 * 20.0 / intake.steps.len() as f64).round()) as u8
    };
    let components = vec![
        component("source-map", source_fields, 30),
        component(
            "human-decisions",
            if intake.human_decisions.is_empty() {
                0
            } else {
                10
            },
            10,
        ),
        component(
            "frequency-and-duration",
            5 + if draft.runs_per_week.is_some() { 5 } else { 0 },
            10,
        ),
        component("step-boundaries", boundary_points, 20),
        component(
            "data-and-side-effects",
            assessed(draft.data_sensitivity != DataSensitivityV1::Unassessed, 5)
                + assessed(draft.side_effect_level != SideEffectLevelV1::Unassessed, 5),
            10,
        ),
        component(
            "verification",
            assessed(draft.verification_mode != VerificationModeV1::Unassessed, 5)
                + assessed(
                    draft.verification_mode == VerificationModeV1::None
                        || draft.verification_minutes_per_run.is_some(),
                    5,
                ),
            10,
        ),
        component(
            "recovery-plan",
            assessed(draft.recovery_plan.is_some(), 10),
            10,
        ),
    ];
    score(components)
}

fn missing_fields(draft: &AutomationBlueprintDraftV1, intake: &WorkflowIntakeV1) -> Vec<String> {
    let classified = draft
        .step_boundaries
        .iter()
        .filter(|boundary| {
            boundary.disposition != StepDispositionV1::Unassessed
                && boundary
                    .rationale
                    .as_ref()
                    .is_some_and(|value| !value.trim().is_empty())
        })
        .count();
    let mut missing = Vec::new();
    if draft.runs_per_week.is_none() {
        missing.push("runsPerWeek".into());
    }
    if classified != intake.steps.len() {
        missing.push("stepBoundaries".into());
    }
    if draft.data_sensitivity == DataSensitivityV1::Unassessed {
        missing.push("dataSensitivity".into());
    }
    if draft.side_effect_level == SideEffectLevelV1::Unassessed {
        missing.push("sideEffectLevel".into());
    }
    if draft.verification_mode == VerificationModeV1::Unassessed
        || (draft.verification_mode != VerificationModeV1::None
            && draft.verification_minutes_per_run.is_none())
    {
        missing.push("verification".into());
    }
    if draft.recovery_plan.is_none() {
        missing.push("recoveryPlan".into());
    }
    missing
}

pub fn estimates(
    draft: &AutomationBlueprintDraftV1,
    intake: &WorkflowIntakeV1,
    evidence: &BlueprintEvidenceV1,
) -> BlueprintEstimatesV1 {
    let total = draft.step_boundaries.len() as f64;
    let automatable = draft
        .step_boundaries
        .iter()
        .filter(|step| step.disposition == StepDispositionV1::Automatable)
        .count();
    let human = draft.step_boundaries.len().saturating_sub(automatable) as f64;
    let values = {
        let runs = draft.runs_per_week.unwrap_or(0);
        let baseline = f64::from(runs) * f64::from(intake.minutes_per_run);
        let verification = f64::from(draft.verification_minutes_per_run.unwrap_or(0));
        let residual = f64::from(runs)
            * ((f64::from(intake.minutes_per_run) * human / total.max(1.0)) + verification);
        let savings = (baseline - residual).max(0.0).round() as u32;
        (baseline.round() as u32, residual.round() as u32, savings)
    };
    let human_share = human / total.max(1.0);
    let tool_complexity = (6 * intake.tools.len().min(6)) as f64;
    let step_complexity = (3 * draft.step_boundaries.len().min(10)) as f64;
    let effort_sensitivity = match draft.data_sensitivity {
        DataSensitivityV1::Unassessed => 8.0,
        DataSensitivityV1::Low => 0.0,
        DataSensitivityV1::Moderate => 4.0,
        DataSensitivityV1::High => 8.0,
    };
    let risk_sensitivity = match draft.data_sensitivity {
        DataSensitivityV1::Unassessed => 20.0,
        DataSensitivityV1::Low => 0.0,
        DataSensitivityV1::Moderate => 10.0,
        DataSensitivityV1::High => 20.0,
    };
    let effort_effect = match draft.side_effect_level {
        SideEffectLevelV1::Unassessed => 12.0,
        SideEffectLevelV1::None => 0.0,
        SideEffectLevelV1::Reversible => 6.0,
        SideEffectLevelV1::Irreversible => 12.0,
    };
    let risk_effect = match draft.side_effect_level {
        SideEffectLevelV1::Unassessed => 28.0,
        SideEffectLevelV1::None => 0.0,
        SideEffectLevelV1::Reversible => 12.0,
        SideEffectLevelV1::Irreversible => 28.0,
    };
    let verification = match draft.verification_mode {
        VerificationModeV1::Unassessed | VerificationModeV1::None => 18.0,
        VerificationModeV1::Sample => 8.0,
        VerificationModeV1::EveryRun => 0.0,
    };
    let recovery_absent = if draft.recovery_plan.is_some() {
        0.0
    } else {
        16.0
    };
    let effort = bounded(
        10.0 + tool_complexity
            + step_complexity
            + 15.0 * human_share
            + effort_sensitivity
            + effort_effect,
    );
    let operational_risk = bounded(
        18.0 * human_share + risk_sensitivity + risk_effect + verification + recovery_absent,
    );
    let completeness = f64::from(completeness(draft, intake).score);
    let observation_component = (6.0 * f64::from(evidence.repeated_observations)).min(30.0);
    let duration_component = (20.0 * evidence.observed_days.min(14.0) / 14.0).round();
    let quality_component = (20.0 * evidence.rules_quality.clamp(0.0, 1.0)).round();
    let runs = f64::from(draft.runs_per_week.unwrap_or(0));
    let frequency_agreement =
        (1.0 - (evidence.observed_weekly_frequency - runs).abs() / runs.max(1.0)).max(0.0);
    let frequency_component = (10.0 * frequency_agreement).round();
    let completeness_component = (20.0 * completeness / 100.0).round();
    let confidence = bounded(
        observation_component
            + duration_component
            + quality_component
            + frequency_component
            + completeness_component,
    );
    let mut components = BTreeMap::new();
    components.insert("toolComplexity".into(), tool_complexity);
    components.insert("stepComplexity".into(), step_complexity);
    components.insert("humanShare".into(), human_share);
    components.insert("effortSensitivity".into(), effort_sensitivity);
    components.insert("effortEffect".into(), effort_effect);
    components.insert("riskSensitivity".into(), risk_sensitivity);
    components.insert("riskEffect".into(), risk_effect);
    components.insert("verification".into(), verification);
    components.insert("recoveryAbsent".into(), recovery_absent);
    components.insert(
        "observations".into(),
        f64::from(evidence.repeated_observations),
    );
    components.insert("observedDays".into(), evidence.observed_days);
    components.insert("rulesQuality".into(), evidence.rules_quality);
    components.insert(
        "observedWeeklyFrequency".into(),
        evidence.observed_weekly_frequency,
    );
    components.insert("frequencyAgreement".into(), frequency_agreement);
    components.insert("completeness".into(), completeness);
    BlueprintEstimatesV1 {
        weekly_baseline_minutes: values.0,
        weekly_residual_minutes: values.1,
        weekly_savings_minutes: values.2,
        effort,
        operational_risk,
        confidence,
        components,
    }
}

fn bounded(value: f64) -> u8 {
    value.round().clamp(0.0, 100.0) as u8
}

fn component(key: &'static str, points: u8, maximum: u8) -> ScoreComponentV1 {
    ScoreComponentV1 {
        key,
        points: points.min(maximum),
        maximum,
    }
}

fn score(components: Vec<ScoreComponentV1>) -> ScoreV1 {
    ScoreV1 {
        score: components
            .iter()
            .map(|component| component.points)
            .sum::<u8>()
            .min(100),
        components,
    }
}

fn assessed(condition: bool, points: u8) -> u8 {
    if condition {
        points
    } else {
        0
    }
}

fn assumptions(draft: &AutomationBlueprintDraftV1, locale: &str) -> Vec<String> {
    let verification = draft.verification_minutes_per_run.unwrap_or(0);
    if locale == "en" {
        vec![
            "Planning estimate: manual time is distributed equally across all declared steps."
                .into(),
            format!("Planning estimate: verification adds {verification} minute(s) to every run."),
            "Savings are neither observed nor guaranteed; validate them before implementation."
                .into(),
        ]
    } else {
        vec![
            "Estimación de planificación: el tiempo manual se distribuye por igual entre todos los pasos declarados.".into(),
            format!("Estimación de planificación: la verificación agrega {verification} minuto(s) a cada ejecución."),
            "El ahorro no es observado ni garantizado; debe validarse antes de implementar.".into(),
        ]
    }
}

fn render(
    draft: &AutomationBlueprintDraftV1,
    source: &BlueprintSourceV1,
    candidate_label: &str,
    evidence: &BlueprintEvidenceV1,
    status: BlueprintStatusV1,
    estimates: &BlueprintEstimatesV1,
    assumptions: &[String],
    locale: &str,
) -> String {
    let (
        heading,
        status_key,
        status_label,
        observations_label,
        savings_label,
        boundary_heading,
        assumptions_heading,
    ) = if locale == "en" {
        (
            "Automation blueprint",
            "Status",
            match status {
                BlueprintStatusV1::Draft => "draft",
                BlueprintStatusV1::Ready => "ready",
                BlueprintStatusV1::Stale => "stale",
                BlueprintStatusV1::Blocked => "blocked",
            },
            "Repeated observations",
            "Estimated weekly savings",
            "Human / automation boundary",
            "Assumptions",
        )
    } else {
        (
            "Blueprint de automatización",
            "Estado",
            match status {
                BlueprintStatusV1::Draft => "borrador",
                BlueprintStatusV1::Ready => "listo",
                BlueprintStatusV1::Stale => "desactualizado",
                BlueprintStatusV1::Blocked => "bloqueado",
            },
            "Observaciones repetidas",
            "Ahorro semanal estimado",
            "Límite humano / automatización",
            "Supuestos",
        )
    };
    let mut markdown = format!(
        "# {heading}: {}\n\n- {status_key}: {status_label}\n- {observations_label}: {}\n",
        candidate_label, evidence.repeated_observations
    );
    markdown.push_str(&format!(
        "- {savings_label}: {} min\n",
        estimates.weekly_savings_minutes
    ));
    markdown.push_str(&format!("\n## {boundary_heading}\n\n"));
    for (position, (step, boundary)) in source.steps.iter().zip(&draft.step_boundaries).enumerate()
    {
        let disposition = match (locale, boundary.disposition) {
            ("en", StepDispositionV1::Unassessed) => "unassessed",
            ("en", StepDispositionV1::Automatable) => "automatable",
            ("en", StepDispositionV1::HumanRequired) => "human-required",
            (_, StepDispositionV1::Unassessed) => "sin evaluar",
            (_, StepDispositionV1::Automatable) => "automatizable",
            (_, StepDispositionV1::HumanRequired) => "requiere intervención humana",
        };
        markdown.push_str(&format!(
            "{}. {} — {disposition}{}\n",
            position + 1,
            step.label,
            boundary
                .rationale
                .as_ref()
                .map(|value| format!(": {value}"))
                .unwrap_or_default()
        ));
    }
    markdown.push_str(&format!("\n## {assumptions_heading}\n\n"));
    for assumption in assumptions {
        markdown.push_str(&format!("- {assumption}\n"));
    }
    markdown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intake() -> WorkflowIntakeV1 {
        let candidate_label = "Weekly report";
        WorkflowIntakeV1 {
            id: "workflow-1".into(),
            candidate_id: crate::models::automation_candidate_id(candidate_label),
            candidate_label: candidate_label.into(),
            trigger: "Friday".into(),
            tools: vec!["Sheets".into(), "Email".into()],
            steps: vec![
                "Collect".into(),
                "Normalize".into(),
                "Review".into(),
                "Send".into(),
            ],
            human_decisions: "Approve exceptions".into(),
            inputs: "Sales".into(),
            desired_output: "Approved report".into(),
            frequency: "Weekly".into(),
            minutes_per_run: 60,
            pain_points: "Manual work".into(),
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-02T00:00:00Z".into(),
        }
    }

    fn ready_draft() -> AutomationBlueprintDraftV1 {
        let intake = intake();
        let mut draft = ephemeral_draft(&intake);
        draft.runs_per_week = Some(5);
        draft.step_boundaries = vec![
            StepBoundaryV1 {
                step_id: "step-01".into(),
                disposition: StepDispositionV1::Automatable,
                rationale: Some("Rules are deterministic".into()),
            },
            StepBoundaryV1 {
                step_id: "step-02".into(),
                disposition: StepDispositionV1::Automatable,
                rationale: Some("Rules are deterministic".into()),
            },
            StepBoundaryV1 {
                step_id: "step-03".into(),
                disposition: StepDispositionV1::HumanRequired,
                rationale: Some("Requires judgment".into()),
            },
            StepBoundaryV1 {
                step_id: "step-04".into(),
                disposition: StepDispositionV1::Automatable,
                rationale: Some("Template output".into()),
            },
        ];
        draft.data_sensitivity = DataSensitivityV1::Low;
        draft.side_effect_level = SideEffectLevelV1::Reversible;
        draft.verification_mode = VerificationModeV1::EveryRun;
        draft.verification_minutes_per_run = Some(5);
        draft.recovery_plan = Some("Restore the prior report and notify the owner".into());
        draft
    }

    fn evidence(
        repeated_observations: u32,
        observed_days: f64,
        rules_quality: f64,
        observed_weekly_frequency: f64,
    ) -> BlueprintEvidenceV1 {
        BlueprintEvidenceV1 {
            observed: vec![],
            declared: vec![],
            repeated_observations,
            observed_days,
            rules_quality,
            observed_weekly_frequency,
        }
    }

    #[test]
    fn wire_contract_is_camel_case_and_rejects_unknown_versions_and_enums() {
        let value = serde_json::to_value(ready_draft()).unwrap();
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["stepBoundaries"][0]["disposition"], "automatable");
        assert!(value.get("schema_version").is_none());
        let mut input = serde_json::json!({
            "draft": value,
            "expectedRevision": 0
        });
        input["draft"]["schemaVersion"] = 2.into();
        let parsed: SaveAutomationBlueprintInputV1 = serde_json::from_value(input.clone()).unwrap();
        assert!(draft_from_input(&intake(), None, parsed.draft).is_err());
        input["draft"]["schemaVersion"] = 1.into();
        input["draft"]["dataSensitivity"] = "secret".into();
        assert!(serde_json::from_value::<SaveAutomationBlueprintInputV1>(input).is_err());

        let blueprint =
            assess_blueprint(ready_draft(), &intake(), 4, 4, 14.0, 0.75, 5.0, "en").unwrap();
        let output = serde_json::to_value(blueprint).unwrap();
        let keys = |value: &serde_json::Value| {
            value
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>()
        };
        assert_eq!(
            keys(&output),
            [
                "completeness",
                "draft",
                "estimates",
                "evidence",
                "renderedMarkdown",
                "schemaVersion",
                "source",
                "status",
            ]
            .into_iter()
            .map(str::to_string)
            .collect()
        );
        assert_eq!(
            keys(&output["source"]),
            [
                "desiredOutput",
                "frequency",
                "humanDecisions",
                "inputs",
                "minutesPerRun",
                "painPoints",
                "steps",
                "tools",
                "trigger",
            ]
            .into_iter()
            .map(str::to_string)
            .collect()
        );
        assert_eq!(
            keys(&output["evidence"]),
            [
                "declared",
                "observed",
                "observedDays",
                "observedWeeklyFrequency",
                "repeatedObservations",
                "rulesQuality",
            ]
            .into_iter()
            .map(str::to_string)
            .collect()
        );
        assert_eq!(
            keys(&output["estimates"]),
            [
                "components",
                "confidence",
                "effort",
                "operationalRisk",
                "weeklyBaselineMinutes",
                "weeklyResidualMinutes",
                "weeklySavingsMinutes",
            ]
            .into_iter()
            .map(str::to_string)
            .collect()
        );
    }

    #[test]
    fn golden_vector_scores_and_estimates_are_stable() {
        let intake = intake();
        let draft = ready_draft();
        assert_eq!(completeness(&draft, &intake).score, 100);
        let values = estimates(&draft, &intake, &evidence(4, 14.0, 0.75, 5.0));
        assert_eq!(values.weekly_baseline_minutes, 300);
        assert_eq!(values.weekly_residual_minutes, 100);
        assert_eq!(values.weekly_savings_minutes, 200);
        assert_eq!(values.effort, 44);
        assert_eq!(values.operational_risk, 17);
        assert_eq!(values.confidence, 89);

        let mut proportional = ephemeral_draft(&intake);
        proportional.runs_per_week = Some(2);
        proportional.step_boundaries[0].disposition = StepDispositionV1::Automatable;
        proportional.step_boundaries[0].rationale = Some("Deterministic".into());
        proportional.data_sensitivity = DataSensitivityV1::Moderate;
        proportional.side_effect_level = SideEffectLevelV1::None;
        proportional.verification_mode = VerificationModeV1::EveryRun;
        proportional.verification_minutes_per_run = Some(0);
        proportional.recovery_plan = Some("Escalate".into());
        let mut two_step_intake = intake.clone();
        two_step_intake.tools.truncate(1);
        two_step_intake.steps.truncate(2);
        proportional.step_boundaries.truncate(2);
        assert_eq!(completeness(&proportional, &two_step_intake).score, 90);
        let proportional_values =
            estimates(&proportional, &two_step_intake, &evidence(2, 7.0, 0.5, 2.0));
        assert_eq!(proportional_values.effort, 34);
        assert_eq!(proportional_values.operational_risk, 19);
        assert_eq!(proportional_values.confidence, 60);
    }

    #[test]
    fn all_scores_and_savings_are_bounded() {
        let intake = intake();
        let mut draft = ready_draft();
        draft.runs_per_week = Some(1_000);
        draft.verification_minutes_per_run = Some(1_440);
        let values = estimates(
            &draft,
            &intake,
            &evidence(u32::MAX, f64::MAX, 20.0, f64::MAX),
        );
        for value in [
            completeness(&draft, &intake).score,
            values.effort,
            values.operational_risk,
            values.confidence,
        ] {
            assert!(value <= 100);
        }
        assert_eq!(
            estimates(&draft, &intake, &evidence(4, 14.0, 1.0, 5.0)).weekly_savings_minutes,
            0
        );
    }

    #[test]
    fn ready_requires_current_source_evidence_and_automation() {
        let intake = intake();
        let draft = ready_draft();
        assert_eq!(
            assess_blueprint(draft.clone(), &intake, 4, 3, 14.0, 0.75, 1.5, "en")
                .unwrap()
                .status,
            BlueprintStatusV1::Ready
        );
        assert_eq!(
            assess_blueprint(draft.clone(), &intake, 1, 1, 7.0, 1.0, 1.0, "en")
                .unwrap()
                .status,
            BlueprintStatusV1::Draft
        );
        let mut changed = intake.clone();
        changed.updated_at = "2025-01-03T00:00:00Z".into();
        assert_eq!(
            assess_blueprint(draft, &changed, 4, 3, 14.0, 0.75, 1.5, "en")
                .unwrap()
                .status,
            BlueprintStatusV1::Stale
        );
    }

    #[test]
    fn evidence_separates_recorded_facts_from_user_declarations() {
        let blueprint =
            assess_blueprint(ready_draft(), &intake(), 4, 3, 2.0, 2.0 / 3.0, 10.5, "en").unwrap();
        let observed = blueprint.evidence.observed.join(" ").to_lowercase();
        assert!(observed.contains("recorded"));
        assert!(observed.contains("interval"));
        assert!(!observed.contains("repetitive"));
        assert!(!observed.contains("rules"));
        assert!(!observed.contains("label"));

        let declared = blueprint.evidence.declared.join(" ").to_lowercase();
        assert!(declared.contains("activity-label"));
        assert!(declared.contains("repetitive"));
        assert!(declared.contains("rules"));
    }

    #[test]
    fn ready_gate_requires_two_user_confirmed_repetitions() {
        let intake = intake();
        let draft = ready_draft();
        assert_eq!(
            assess_blueprint(draft.clone(), &intake, 4, 0, 0.0, 0.0, 0.0, "en")
                .unwrap()
                .status,
            BlueprintStatusV1::Draft
        );
        assert_eq!(
            assess_blueprint(draft, &intake, 4, 2, 2.0, 1.0, 7.0, "en")
                .unwrap()
                .status,
            BlueprintStatusV1::Ready
        );
    }

    #[test]
    fn validation_rejects_controls_bounds_and_wrong_step_ids() {
        let intake = intake();
        let mut draft = ready_draft();
        draft.runs_per_week = Some(1_001);
        assert!(validate_draft(&draft, &intake).is_err());
        draft.runs_per_week = Some(1);
        draft.step_boundaries[0].rationale = Some("bad\u{0007}".into());
        assert!(validate_draft(&draft, &intake).is_err());
        draft.step_boundaries[0].rationale = Some("ok".into());
        draft.step_boundaries[0].step_id = "step-02".into();
        assert!(validate_draft(&draft, &intake).is_err());
    }

    #[test]
    fn whitespace_rationales_are_normalized_or_rejected_and_never_ready() {
        let intake = intake();
        let mut whitespace = ready_draft();
        whitespace.step_boundaries[0].rationale = Some(" \n\t ".into());
        assert!(completeness(&whitespace, &intake).score < 100);
        assert!(assess_blueprint(whitespace.clone(), &intake, 2, 2, 2.0, 1.0, 7.0, "en").is_err());
        assert!(draft_from_input(&intake, None, whitespace).is_err());

        let mut spaced = ready_draft();
        spaced.step_boundaries[0].rationale = Some("  Clear   deterministic\n rules  ".into());
        let normalized = draft_from_input(&intake, None, spaced).unwrap();
        assert_eq!(
            normalized.step_boundaries[0].rationale.as_deref(),
            Some("Clear deterministic rules")
        );
    }

    #[test]
    fn refresh_preserves_declared_controls_and_resets_all_step_classifications() {
        let mut changed = intake();
        changed.updated_at = "2025-01-03T00:00:00Z".into();
        changed.steps = vec!["Collect".into(), "Approve".into()];
        let refreshed = refresh_draft(ready_draft(), &changed).unwrap();
        assert_eq!(refreshed.runs_per_week, Some(5));
        assert_eq!(refreshed.data_sensitivity, DataSensitivityV1::Low);
        assert_eq!(refreshed.side_effect_level, SideEffectLevelV1::Reversible);
        assert_eq!(refreshed.verification_mode, VerificationModeV1::EveryRun);
        assert_eq!(refreshed.verification_minutes_per_run, Some(5));
        assert!(refreshed.recovery_plan.is_some());
        assert_eq!(
            refreshed
                .step_boundaries
                .iter()
                .map(|boundary| boundary.step_id.as_str())
                .collect::<Vec<_>>(),
            ["step-01", "step-02"]
        );
        assert!(refreshed.step_boundaries.iter().all(|boundary| {
            boundary.disposition == StepDispositionV1::Unassessed && boundary.rationale.is_none()
        }));
    }
}
