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

const bounded = (value: number) => Math.max(0, Math.min(100, Math.round(value)));
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
    id: `blueprint-${intake.id}`,
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
  const verification = draft.verificationMinutesPerRun ?? 0;
  const weeklyBaselineMinutes = Math.round(runs * source.minutesPerRun);
  const weeklyResidualMinutes = Math.round(runs * ((source.minutesPerRun * human / total) + verification));
  const weeklySavingsMinutes = Math.max(0, Math.round(weeklyBaselineMinutes - weeklyResidualMinutes));
  const sensitivity: Record<DataSensitivityV1, number> = { unassessed: 25, low: 5, moderate: 15, high: 30 };
  const sideEffects: Record<SideEffectLevelV1, number> = { unassessed: 25, none: 0, reversible: 8, irreversible: 35 };
  const verificationBurden: Record<VerificationModeV1, number> = { unassessed: 20, none: 0, sample: 7, "every-run": 4 };
  const complexity = bounded(24 + (total * 5));
  const effort = bounded(complexity + (draft.sideEffectLevel === "irreversible" ? 20 : 0));
  const operationalRisk = bounded(sensitivity[draft.dataSensitivity] + sideEffects[draft.sideEffectLevel] + verificationBurden[draft.verificationMode] + (hasText(draft.recoveryPlan) ? 0 : 20));
  const confidence = bounded(31 + Math.min(18, evidence.repeatedObservations * 4.5) + (draft.stepBoundaries.filter((item) => item.disposition !== "unassessed").length === total ? 20 : 0) + (draft.verificationMode !== "unassessed" ? 5 : 0) + (hasText(draft.recoveryPlan) ? 15 : 0));
  return { weeklyBaselineMinutes, weeklyResidualMinutes, weeklySavingsMinutes, effort, operationalRisk, confidence, components: { complexity, sensitivity: sensitivity[draft.dataSensitivity], sideEffects: sideEffects[draft.sideEffectLevel], verification: verificationBurden[draft.verificationMode], observations: evidence.repeatedObservations } };
}

export function buildPreviewBlueprint(intake: WorkflowIntakeV1, draft = emptyBlueprintDraft(intake), evidence: BlueprintEvidenceV1 = { observed: ["Repeated check-in activity", "Local timeline pattern"], declared: ["Workflow intake supplied by you"], repeatedObservations: 3 }): AutomationBlueprintV1 {
  const source = sourceFromIntake(intake);
  const completeness = computeCompleteness(draft, source);
  const estimates = computeEstimates(draft, source, evidence);
  const sourceCurrent = draft.sourceIntakeUpdatedAt === intake.updatedAt;
  const status = !source.steps.length ? "blocked" : !sourceCurrent ? "stale" : completeness.score === 100 && evidence.repeatedObservations >= 2 && draft.stepBoundaries.some((item) => item.disposition === "automatable") ? "ready" : "draft";
  const lines = ["# Automation blueprint", "", `Status: ${status}`, `Completeness: ${completeness.score}/100`, "", "## Assumptions", "- Equal time per step.", "- Verification overhead is included.", "- Savings are planning estimates, not observed outcomes."];
  return { schemaVersion: 1, draft, source, evidence, status, completeness, estimates, renderedMarkdown: lines.join("\n") };
}
