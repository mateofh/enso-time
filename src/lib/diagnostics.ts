import type { AutomationCandidateV1, CheckInV1, DiagnosticV1, TimelineBlockV1 } from "./types";
import { candidateId, canonicalCandidateLabel, compareUtf8 } from "./candidateIdentity";

export { candidateId } from "./candidateIdentity";

const WEEK_DAYS = 5;

function round(value: number, digits = 1): number {
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
}

interface CandidateObservation {
  label: string;
  durationSeconds: number;
  rulesClear: boolean;
  source: "corrección" | "regla" | "check-in";
}

function fallbackActivityLabel(category: string, app: string): string {
  const labels: Record<string, string> = {
    focus: "Foco", communication: "Comunicación", meetings: "Reuniones", admin: "Administración",
    learning: "Aprendizaje", creation: "Creación", personal: "Personal",
  };
  return `${labels[category] ?? "Actividad"} · ${app.trim()}`;
}

export function scoreCandidate(checkIns: CheckInV1[], daysObserved: number, blocks: TimelineBlockV1[] = []): AutomationCandidateV1[] {
  const groups = new Map<string, CandidateObservation[]>();
  const representedCheckIns = new Set(blocks.flatMap((block) => block.checkInId ? [block.checkInId] : []));
  for (const block of blocks) {
    if (block.afk || block.category === "unknown" || block.category === "break") continue;
    if (!block.classificationSource || !["override", "rule", "checkin"].includes(block.classificationSource)) continue;
    const label = block.activityLabel?.trim() || fallbackActivityLabel(block.category, block.app);
    const key = canonicalCandidateLabel(label);
    const source = block.classificationSource === "override" ? "corrección" : block.classificationSource === "rule" ? "regla" : "check-in";
    groups.set(key, [...(groups.get(key) ?? []), {
      label,
      durationSeconds: Math.max(0, block.durationSeconds),
      rulesClear: block.classificationSource === "rule",
      source,
    }]);
  }
  for (const checkIn of checkIns) {
    if (checkIn.status !== "answered" || !checkIn.activityLabel || !checkIn.repetitive || representedCheckIns.has(checkIn.id)) continue;
    const key = canonicalCandidateLabel(checkIn.activityLabel);
    if (!key) continue;
    const durationSeconds = Math.max(0, (Date.parse(checkIn.intervalEnd) - Date.parse(checkIn.intervalStart)) / 1000) || 0;
    groups.set(key, [...(groups.get(key) ?? []), {
      label: checkIn.activityLabel.trim(), durationSeconds, rulesClear: checkIn.rulesClear === true, source: "check-in",
    }]);
  }

  return [...groups.entries()]
    .filter(([, values]) => values.length >= 2)
    .map(([key, values]) => {
      const weeklyFrequency = round((values.length / Math.max(daysObserved, 1)) * WEEK_DAYS);
      const estimatedWeeklyHours = round((values.reduce((sum, value) => sum + value.durationSeconds, 0) / 3600 / Math.max(daysObserved, 1)) * WEEK_DAYS);
      const impactScore = Math.min(40, Math.round(estimatedWeeklyHours * 5 + weeklyFrequency * 1.5));
      const clearRatio = values.filter((value) => value.rulesClear).length / values.length;
      const feasibilityScore = Math.round(18 + clearRatio * 17);
      const riskScore = clearRatio > 0.6 ? 6 : 14;
      const activityLabel = values.map((value) => value.label).sort(compareUtf8)[0] ?? key;
      const corrections = values.filter((value) => value.source === "corrección").length;
      const rules = values.filter((value) => value.source === "regla").length;
      const checkInCount = values.filter((value) => value.source === "check-in").length;
      return {
        id: candidateId(key),
        activityLabel,
        estimatedWeeklyHours,
        weeklyFrequency,
        impactScore,
        feasibilityScore,
        riskScore,
        priorityScore: Math.max(0, Math.min(100, impactScore + feasibilityScore - riskScore)),
        evidence: [
          `${values.length} sesiones: ${corrections} correcciones, ${rules} reglas y ${checkInCount} check-ins`,
          `${weeklyFrequency} repeticiones estimadas por semana`,
          `${estimatedWeeklyHours} horas estimadas por semana`,
          clearRatio > 0.6 ? "La mayoría sigue reglas claras" : "Necesita aclarar reglas antes de automatizar",
        ],
      };
    })
    .sort((a, b) => b.priorityScore - a.priorityScore || compareUtf8(a.id, b.id) || compareUtf8(a.activityLabel, b.activityLabel));
}

export function buildPrompt(candidate: AutomationCandidateV1 | undefined, role: string, goals: string[], daysObserved: number): string {
  if (!candidate) {
    return `Actúa como consultor de automatización. Todavía no hay suficiente evidencia para elegir una tarea. Ayúdame a diseñar un estudio de trabajo de ${daysObserved} días para identificar procesos repetitivos, medibles y de bajo riesgo. Mi rol es: ${role || "profesional independiente"}. Mis objetivos son: ${goals.join(", ") || "recuperar tiempo para trabajo de mayor valor"}.`;
  }

  return `Actúa como arquitecto de automatización pragmático. Quiero decidir qué automatizar primero basándome en evidencia de mi trabajo, no en tendencias.\n\nCONTEXTO\n- Rol: ${role.trim() || "profesional independiente"}\n- Objetivos: ${goals.join(", ") || "recuperar tiempo para trabajo de mayor valor"}\n- Días observados: ${daysObserved}\n\nCANDIDATO PRINCIPAL\n- Actividad: ${candidate.activityLabel}\n- Frecuencia estimada: ${candidate.weeklyFrequency} veces por semana\n- Tiempo estimado: ${candidate.estimatedWeeklyHours} horas por semana\n- Impacto: ${candidate.impactScore}/40\n- Facilidad: ${candidate.feasibilityScore}/35\n- Riesgo: ${candidate.riskScore}/25\n- Evidencia observada: ${candidate.evidence.join("; ")}\n\nENTREGABLE\n1. Diagnostica si realmente conviene automatizar esta actividad primero.\n2. Separa pasos deterministas de pasos que requieren criterio humano.\n3. Propón un MVP que pueda validar en menos de una semana.\n4. Evalúa las herramientas declaradas y recomienda una arquitectura simple sólo si la evidencia la sustenta.\n5. Define riesgos, controles y una métrica de éxito en horas ahorradas.\n6. Si no es el mejor primer candidato, explica qué información falta antes de descartarlo.\n\nNo inventes datos ni trates el intake autodeclarado como evidencia observada. Señala cada supuesto. No generes código, credenciales ni configuración ejecutable, y no construyas ni despliegues ningún flujo; entrega sólo el diagnóstico y el diseño reversible.`;
}

export function createDiagnostic(
  blocks: TimelineBlockV1[],
  checkIns: CheckInV1[],
  daysObserved: number,
  role = "",
  goals: string[] = [],
): DiagnosticV1 {
  const activeSeconds = blocks.filter((block) => !block.afk).reduce((sum, block) => sum + block.durationSeconds, 0);
  const focusSeconds = blocks.filter((block) => block.category === "focus").reduce((sum, block) => sum + block.durationSeconds, 0);
  const switches = blocks.reduce((count, block, index) => {
    const previous = blocks[index - 1];
    return count + (previous && previous.app !== block.app ? 1 : 0);
  }, 0);
  const answered = checkIns.filter((checkIn) => checkIn.status === "answered").length;
  const candidates = scoreCandidate(checkIns, daysObserved, blocks);
  const recommendations: string[] = [];

  if (activeSeconds > 0 && focusSeconds / activeSeconds < 0.35) recommendations.push("Reserva un bloque protegido de 45 minutos antes de abrir comunicación.");
  if (switches > Math.max(8, activeSeconds / 3600 * 12)) recommendations.push("Agrupa mensajes y tareas administrativas en dos ventanas del día.");
  if (checkIns.some((checkIn) => checkIn.status === "pending")) recommendations.push("Resuelve los intervalos pendientes para mejorar la calidad del diagnóstico.");
  if (candidates[0]) recommendations.push(`Valida “${candidates[0].activityLabel}” como primer experimento de automatización.`);
  if (recommendations.length === 0) recommendations.push("Sigue registrando contexto; el patrón actual todavía es estable y no exige un cambio inmediato.");

  return {
    schemaVersion: 1,
    rangeStart: blocks[0]?.start ?? new Date().toISOString(),
    rangeEnd: blocks.at(-1)?.end ?? new Date().toISOString(),
    daysObserved,
    metrics: [
      { label: "Tiempo activo", value: activeSeconds, unit: "seconds" },
      { label: "Trabajo enfocado", value: activeSeconds ? round((focusSeconds / activeSeconds) * 100, 0) : 0, unit: "percent" },
      { label: "Cambios de contexto", value: switches, unit: "count" },
      { label: "Check-ins respondidos", value: answered, unit: "count" },
    ],
    recommendations: recommendations.slice(0, 3),
    candidates,
    prompt: buildPrompt(candidates[0], role, goals, daysObserved),
    preliminary: daysObserved < 7,
    generatedAt: new Date().toISOString(),
  };
}
