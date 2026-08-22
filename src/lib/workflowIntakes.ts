import type { WorkflowIntakeV1 } from "./types";

export const MAX_WORKFLOW_INTAKES = 500;
export const MAX_WORKFLOW_MINUTES = 1_440;

function characterCount(value: string): number {
  return [...value].length;
}

function normalizeText(value: string): string {
  return value.trim().split(/\s+/u).filter(Boolean).join(" ");
}

function asciiFold(value: string): string {
  return value.replace(/[A-Z]/g, (character) => character.toLowerCase());
}

function normalizeUniqueTools(values: string[]): string[] {
  const normalized: string[] = [];
  for (const value of values) {
    const next = normalizeText(value);
    if (!normalized.some((existing) => asciiFold(existing) === asciiFold(next))) normalized.push(next);
  }
  return normalized;
}

function validateRawText(value: unknown, max: number, label: string): asserts value is string {
  if (typeof value !== "string" || characterCount(value) > max) throw new Error(`${label} excede ${max} caracteres`);
}

function validateIdentifier(value: string, label: string) {
  if (!/^[A-Za-z0-9_.:-]{1,128}$/.test(value)) throw new Error(`${label} debe tener entre 1 y 128 caracteres ASCII seguros`);
}

function validateRequiredText(value: string, max: number, label: string) {
  if (!value || characterCount(value) > max || /\p{Cc}/u.test(value)) throw new Error(`${label} debe tener entre 1 y ${max} caracteres sin controles`);
}

function normalizeUtc(value: string, label: string): { value: string; milliseconds: number } {
  const milliseconds = Date.parse(value);
  if (!Number.isFinite(milliseconds)) throw new Error(`${label} no es una fecha RFC3339 válida`);
  const normalized = new Date(milliseconds).toISOString().replace(/\.000Z$/, "+00:00").replace(/Z$/, "+00:00");
  return { value: normalized, milliseconds };
}

export function normalizeWorkflowIntake(intake: WorkflowIntakeV1, now = new Date()): WorkflowIntakeV1 {
  for (const [value, max, label] of [
    [intake.id, 128, "El id del intake"],
    [intake.candidateId, 128, "El id del candidato"],
    [intake.candidateLabel, 200, "El nombre del candidato"],
    [intake.trigger, 500, "El disparador"],
    [intake.humanDecisions, 2_000, "Las decisiones humanas"],
    [intake.inputs, 2_000, "Las entradas"],
    [intake.desiredOutput, 2_000, "El resultado deseado"],
    [intake.frequency, 200, "La frecuencia"],
    [intake.painPoints, 2_000, "Los puntos de dolor"],
    [intake.createdAt, 64, "createdAt"],
    [intake.updatedAt, 64, "updatedAt"],
  ] as const) validateRawText(value, max, label);

  if (!Array.isArray(intake.tools) || intake.tools.length > 25 || intake.tools.some((tool) => typeof tool !== "string" || characterCount(tool) > 100)) {
    throw new Error("Las herramientas exceden el tamaño permitido");
  }
  if (!Array.isArray(intake.steps) || intake.steps.length > 50 || intake.steps.some((step) => typeof step !== "string" || characterCount(step) > 500)) {
    throw new Error("Los pasos exceden el tamaño permitido");
  }

  const createdAt = normalizeUtc(intake.createdAt, "createdAt");
  const updatedAt = normalizeUtc(intake.updatedAt, "updatedAt");
  const normalized: WorkflowIntakeV1 = {
    ...intake,
    id: normalizeText(intake.id),
    candidateId: normalizeText(intake.candidateId),
    candidateLabel: normalizeText(intake.candidateLabel),
    trigger: normalizeText(intake.trigger),
    tools: normalizeUniqueTools(intake.tools),
    steps: intake.steps.map(normalizeText),
    humanDecisions: normalizeText(intake.humanDecisions),
    inputs: normalizeText(intake.inputs),
    desiredOutput: normalizeText(intake.desiredOutput),
    frequency: normalizeText(intake.frequency),
    minutesPerRun: intake.minutesPerRun,
    painPoints: normalizeText(intake.painPoints),
    createdAt: createdAt.value,
    updatedAt: updatedAt.value,
  };

  validateIdentifier(normalized.id, "El id del intake");
  validateIdentifier(normalized.candidateId, "El id del candidato");
  validateRequiredText(normalized.candidateLabel, 200, "El nombre del candidato");
  validateRequiredText(normalized.trigger, 500, "El disparador");
  validateRequiredText(normalized.humanDecisions, 2_000, "Las decisiones humanas");
  validateRequiredText(normalized.inputs, 2_000, "Las entradas");
  validateRequiredText(normalized.desiredOutput, 2_000, "El resultado deseado");
  validateRequiredText(normalized.frequency, 200, "La frecuencia");
  validateRequiredText(normalized.painPoints, 2_000, "Los puntos de dolor");
  if (!Number.isInteger(normalized.minutesPerRun) || normalized.minutesPerRun < 1 || normalized.minutesPerRun > MAX_WORKFLOW_MINUTES) {
    throw new Error("Los minutos por ejecución deben estar entre 1 y 1440");
  }
  if (normalized.tools.length < 1 || normalized.tools.length > 25) throw new Error("Debe declarar entre 1 y 25 herramientas");
  normalized.tools.forEach((tool) => validateRequiredText(tool, 100, "Una herramienta"));
  if (normalized.steps.length < 1 || normalized.steps.length > 50) throw new Error("Debe declarar entre 1 y 50 pasos");
  normalized.steps.forEach((step) => validateRequiredText(step, 500, "Un paso"));
  if (updatedAt.milliseconds < createdAt.milliseconds) throw new Error("updatedAt no puede ser anterior a createdAt");
  const latestAllowed = now.getTime() + 5 * 60_000;
  if (createdAt.milliseconds > latestAllowed || updatedAt.milliseconds > latestAllowed) throw new Error("Las fechas del intake no pueden estar en el futuro");
  return normalized;
}

export function normalizeWorkflowIntakes(intakes: WorkflowIntakeV1[], now = new Date()): WorkflowIntakeV1[] {
  return intakes.map((intake) => normalizeWorkflowIntake(intake, now)).sort((left, right) => {
    const timestamp = Date.parse(right.updatedAt) - Date.parse(left.updatedAt);
    return timestamp || (left.id < right.id ? -1 : left.id > right.id ? 1 : 0);
  });
}

export function upsertWorkflowIntake(current: WorkflowIntakeV1[], incoming: WorkflowIntakeV1, now = new Date()): { items: WorkflowIntakeV1[]; persisted: WorkflowIntakeV1 } {
  const items = normalizeWorkflowIntakes(current, now);
  const candidate = normalizeWorkflowIntake(incoming, now);
  const existingIndex = items.findIndex((item) => item.candidateId === candidate.candidateId);
  if (existingIndex < 0) {
    if (items.length >= MAX_WORKFLOW_INTAKES) throw new Error("No se pueden guardar más de 500 intakes de workflow");
    return { items: normalizeWorkflowIntakes([...items, candidate], now), persisted: candidate };
  }

  const existing = items[existingIndex]!;
  if (Date.parse(candidate.updatedAt) < Date.parse(existing.updatedAt)) return { items, persisted: existing };
  const persisted = { ...candidate, id: existing.id, createdAt: existing.createdAt };
  const updated = items.map((item, index) => index === existingIndex ? persisted : item);
  return { items: normalizeWorkflowIntakes(updated, now), persisted };
}
