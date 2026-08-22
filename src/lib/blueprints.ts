import type {
  AutomationBlueprintDraftV1,
  AutomationBlueprintV1,
  BlueprintEvidenceV1,
  BlueprintSourceMapV1,
  DataSensitivityV1,
  SideEffectLevelV1,
  VerificationModeV1,
  WorkflowIntakeV1,
} from "./types";

const clamp = (value: number) => Math.max(0, Math.min(100, value));
const bounded = (value: number) => Math.round(clamp(value));
const hasText = (value: string | undefined) => Boolean(value?.trim());

export function sourceFromIntake(intake: WorkflowIntakeV1): BlueprintSourceMapV1 {
  return {
    trigger: intake.trigger,
    inputs: intake.inputs,
    desiredOutput: intake.desiredOutput,
    tools: intake.tools,
    steps: intake.steps.map((label, index) => ({ id: `step-${String(index + 1).padStart(2, "0")}`, label })),
    humanDecisions: intake.humanDecisions,
    minutesPerRun: intake.minutesPerRun,
    frequency: intake.frequency,
    painPoints: intake.painPoints,
  };
}

export function emptyBlueprintDraft(intake: WorkflowIntakeV1): AutomationBlueprintDraftV1 {
  const now = new Date().toISOString();
  return {
    schemaVersion: 1,
    id: `blueprint-${intake.candidateId}`,
    candidateId: intake.candidateId,
    workflowIntakeId: intake.id,
    sourceIntakeUpdatedAt: intake.updatedAt,
    revision: 0,
    stepBoundaries: sourceFromIntake(intake).steps.map((step) => ({ stepId: step.id, disposition: "unassessed" })),
    dataSensitivity: "unassessed",
    sideEffectLevel: "unassessed",
    verificationMode: "unassessed",
    createdAt: now,
    updatedAt: now,
  };
}

export function computeCompleteness(draft: AutomationBlueprintDraftV1, source: BlueprintSourceMapV1) {
  const sourceFields = [source.trigger, source.inputs, source.desiredOutput, source.tools.length ? "tools" : "", source.steps.length ? "steps" : ""];
  let score = sourceFields.filter(hasText).length * 6;
  if (hasText(source.humanDecisions)) score += 10;
  if (source.minutesPerRun > 0) score += 5;
  if (draft.runsPerWeek && draft.runsPerWeek > 0) score += 5;
  const classified = draft.stepBoundaries.filter((item) => item.disposition !== "unassessed" && hasText(item.rationale)).length;
  score += source.steps.length ? 20 * classified / source.steps.length : 0;
  if (draft.dataSensitivity !== "unassessed") score += 5;
  if (draft.sideEffectLevel !== "unassessed") score += 5;
  if (draft.verificationMode !== "unassessed") score += 5;
  if (draft.verificationMode === "none" || (draft.verificationMinutesPerRun !== undefined && draft.verificationMinutesPerRun >= 0)) score += 5;
  if (hasText(draft.recoveryPlan)) score += 10;
  const missing: string[] = [];
  if (!draft.runsPerWeek) missing.push("runsPerWeek");
  if (classified !== source.steps.length) missing.push("stepBoundaries");
  if (draft.dataSensitivity === "unassessed") missing.push("dataSensitivity");
  if (draft.sideEffectLevel === "unassessed") missing.push("sideEffectLevel");
  if (draft.verificationMode === "unassessed" || (draft.verificationMode !== "none" && draft.verificationMinutesPerRun === undefined)) missing.push("verification");
  if (!hasText(draft.recoveryPlan)) missing.push("recoveryPlan");
  return { score: bounded(score), missing };
}

export function computeEstimates(draft: AutomationBlueprintDraftV1, source: BlueprintSourceMapV1, evidence: BlueprintEvidenceV1) {
  const total = source.steps.length || 1;
  const automatable = draft.stepBoundaries.filter((item) => item.disposition === "automatable").length;
  const human = total - automatable;
  const runs = draft.runsPerWeek ?? 0;
  const verificationMinutes = draft.verificationMinutesPerRun ?? 0;
  const weeklyBaselineMinutes = Math.round(runs * source.minutesPerRun);
  const weeklyResidualMinutes = Math.round(runs * ((source.minutesPerRun * human / total) + verificationMinutes));
  const weeklySavingsMinutes = Math.max(0, Math.round(weeklyBaselineMinutes - weeklyResidualMinutes));
  const effortSensitivity: Record<DataSensitivityV1, number> = { unassessed: 8, low: 0, moderate: 4, high: 8 };
  const riskSensitivity: Record<DataSensitivityV1, number> = { unassessed: 20, low: 0, moderate: 10, high: 20 };
  const effortEffect: Record<SideEffectLevelV1, number> = { unassessed: 12, none: 0, reversible: 6, irreversible: 12 };
  const riskEffect: Record<SideEffectLevelV1, number> = { unassessed: 28, none: 0, reversible: 12, irreversible: 28 };
  const verification: Record<VerificationModeV1, number> = { unassessed: 18, none: 18, sample: 8, "every-run": 0 };
  const humanShare = human / total;
  const toolComplexity = 6 * Math.min(source.tools.length, 6);
  const stepComplexity = 3 * Math.min(total, 10);
  const effort = bounded(10 + toolComplexity + stepComplexity + (15 * humanShare) + effortSensitivity[draft.dataSensitivity] + effortEffect[draft.sideEffectLevel]);
  const recoveryAbsent = hasText(draft.recoveryPlan) ? 0 : 16;
  const operationalRisk = bounded((18 * humanShare) + riskSensitivity[draft.dataSensitivity] + riskEffect[draft.sideEffectLevel] + verification[draft.verificationMode] + recoveryAbsent);
  const completeness = computeCompleteness(draft, source).score;
  const observationComponent = Math.min(30, 6 * evidence.repeatedObservations);
  const durationComponent = Math.round(20 * Math.min(evidence.observedDays, 14) / 14);
  const qualityComponent = Math.round(20 * clamp(evidence.rulesQuality));
  const frequencyAgreement = Math.max(0, 1 - Math.abs(evidence.observedWeeklyFrequency - runs) / Math.max(1, runs));
  const frequencyComponent = Math.round(10 * frequencyAgreement);
  const completenessComponent = Math.round(20 * completeness / 100);
  const confidence = bounded(observationComponent + durationComponent + qualityComponent + frequencyComponent + completenessComponent);
  return { weeklyBaselineMinutes, weeklyResidualMinutes, weeklySavingsMinutes, effort, operationalRisk, confidence, components: { toolComplexity, stepComplexity, humanShare, effortSensitivity: effortSensitivity[draft.dataSensitivity], effortEffect: effortEffect[draft.sideEffectLevel], riskSensitivity: riskSensitivity[draft.dataSensitivity], riskEffect: riskEffect[draft.sideEffectLevel], verification: verification[draft.verificationMode], recoveryAbsent, observations: evidence.repeatedObservations, observedDays: evidence.observedDays, rulesQuality: evidence.rulesQuality, observedWeeklyFrequency: evidence.observedWeeklyFrequency, frequencyAgreement, completeness } };
}

export function buildPreviewBlueprint(intake: WorkflowIntakeV1, draft = emptyBlueprintDraft(intake), evidence: BlueprintEvidenceV1 = { observed: ["Recorded check-in interval"], declared: ["Activity, repetition, and rules confirmed by you"], repeatedObservations: 3, observedDays: 7, rulesQuality: 1, observedWeeklyFrequency: draft.runsPerWeek ?? 0 }): AutomationBlueprintV1 {
  const source = sourceFromIntake(intake);
  const completeness = computeCompleteness(draft, source);
  const estimates = computeEstimates(draft, source, evidence);
  const sourceCurrent = draft.sourceIntakeUpdatedAt === intake.updatedAt;
  const status = !source.steps.length ? "blocked" : !sourceCurrent ? "stale" : completeness.score === 100 && evidence.repeatedObservations >= 2 && draft.stepBoundaries.some((item) => item.disposition === "automatable") ? "ready" : "draft";
  const lines = ["# Automation blueprint", "", `Status: ${status}`, `Completeness: ${completeness.score}/100`, "", "## Assumptions", "- Equal time per step.", "- Verification overhead is included.", "- Savings are planning estimates, not observed outcomes."];
  return { schemaVersion: 1, draft, source, evidence, status, completeness, estimates, renderedMarkdown: lines.join("\n") };
}
