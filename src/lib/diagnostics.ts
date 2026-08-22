import type { AutomationCandidateV1, CheckInV1, DiagnosticV1, TimelineBlockV1 } from "./types";

const WEEK_DAYS = 5;
const FNV_OFFSET = BigInt("0xcbf29ce484222325");
const FNV_PRIME = BigInt("0x100000001b3");

function round(value: number, digits = 1): number {
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
}

export function scoreCandidate(checkIns: CheckInV1[], daysObserved: number): AutomationCandidateV1[] {
  const groups = new Map<string, CheckInV1[]>();
  for (const checkIn of checkIns) {
    if (checkIn.status !== "answered" || !checkIn.activityLabel || !checkIn.repetitive) continue;
    const key = checkIn.activityLabel.trim().toLowerCase();
    groups.set(key, [...(groups.get(key) ?? []), checkIn]);
  }

  return [...groups.entries()]
    .filter(([, values]) => values.length >= 2)
    .map(([key, values]) => {
      const weeklyFrequency = round((values.length / Math.max(daysObserved, 1)) * WEEK_DAYS);
      const estimatedWeeklyHours = round(weeklyFrequency * 0.25);
      const impactScore = Math.min(40, Math.round(estimatedWeeklyHours * 5 + weeklyFrequency * 1.5));
      const clearRatio = values.filter((value) => value.rulesClear).length / values.length;
      const feasibilityScore = Math.round(18 + clearRatio * 17);
      const riskScore = clearRatio > 0.6 ? 6 : 14;
      return {
        id: candidateId(key),
        activityLabel: values[0]?.activityLabel ?? key,
        estimatedWeeklyHours,
        weeklyFrequency,
        impactScore,
        feasibilityScore,
        riskScore,
        priorityScore: Math.max(0, Math.min(100, impactScore + feasibilityScore - riskScore)),
        evidence: [
          `${values.length} registros durante el periodo`,
          `${weeklyFrequency} repeticiones estimadas por semana`,
          clearRatio > 0.6 ? "La mayoría sigue reglas claras" : "Necesita aclarar reglas antes de automatizar",
        ],
      };
    })
    .sort((a, b) => b.priorityScore - a.priorityScore || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
}

export function candidateId(label: string): string {
  let hash = FNV_OFFSET;
  for (const byte of new TextEncoder().encode(label.trim())) {
    hash = BigInt.asUintN(64, (hash ^ BigInt(byte)) * FNV_PRIME);
  }
  return `candidate-${hash.toString(16).padStart(16, "0")}`;
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
  const candidates = scoreCandidate(checkIns, daysObserved);
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
