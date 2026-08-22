import { describe, expect, it } from "vitest";
import { buildPreviewBlueprint, computeCompleteness, computeEstimates, emptyBlueprintDraft, sourceFromIntake } from "./blueprints";
import type { WorkflowIntakeV1 } from "./types";

function intake(steps = ["Export", "Validate", "Review", "Publish"], minutesPerRun = 60): WorkflowIntakeV1 {
  return {
    id: "workflow-vectors", candidateId: "candidate-vectors", candidateLabel: "Weekly report", trigger: "Weekly sales close arrives", tools: ["Spreadsheet", "Internal messaging"], steps,
    humanDecisions: "Exclude disputed accounts", inputs: "Sales CSV and targets table", desiredOutput: "Validated report", frequency: "Weekly", minutesPerRun, painPoints: "Manual formatting",
    createdAt: "2026-08-20T12:00:00.000Z", updatedAt: "2026-08-21T12:00:00.000Z",
  };
}

describe("automation blueprint preview golden vectors", () => {
  it("derives the stable blueprint id from the candidate, not the intake id", () => {
    const sourceIntake = intake();
    sourceIntake.id = "workflow-revision-two";
    sourceIntake.candidateId = "candidate-stable-id";
    expect(emptyBlueprintDraft(sourceIntake).id).toBe("blueprint-candidate-stable-id");
  });

  it("keeps recorded check-in evidence separate from user confirmations", () => {
    const blueprint = buildPreviewBlueprint(intake());
    expect(blueprint.evidence.observed).toEqual(["Recorded check-in interval"]);
    expect(blueprint.evidence.declared).toEqual(["Activity, repetition, and rules confirmed by you"]);
    expect(blueprint.evidence.declaredRepeatedDays).toBe(7);
  });

  it("matches the complete four-step workflow vector", () => {
    const sourceIntake = intake();
    const draft = {
      ...emptyBlueprintDraft(sourceIntake), runsPerWeek: 5, dataSensitivity: "low" as const, sideEffectLevel: "reversible" as const, verificationMode: "every-run" as const, verificationMinutesPerRun: 5, recoveryPlan: "Review the CSV and repeat validation",
      stepBoundaries: ["automatable", "automatable", "automatable", "human-required"].map((disposition, index) => ({ stepId: `step-${String(index + 1).padStart(2, "0")}`, disposition: disposition as "automatable" | "human-required", rationale: "Documented boundary" })),
    };
    const blueprint = buildPreviewBlueprint(sourceIntake, draft, { observed: ["a", "b", "c", "d"], declared: ["intake"], repeatedObservations: 4, observedDays: 14, declaredRepeatedDays: 14, rulesQuality: 0.75, observedWeeklyFrequency: 5 });
    expect(blueprint).toMatchObject({ status: "ready", completeness: { score: 100 }, estimates: { weeklyBaselineMinutes: 300, weeklyResidualMinutes: 100, weeklySavingsMinutes: 200, effort: 44, operationalRisk: 17, confidence: 89 } });
  });

  it("keeps a complete intake as a draft until a second observed repetition", () => {
    const sourceIntake = intake();
    const draft = {
      ...emptyBlueprintDraft(sourceIntake), runsPerWeek: 5, dataSensitivity: "low" as const, sideEffectLevel: "reversible" as const, verificationMode: "every-run" as const, verificationMinutesPerRun: 5, recoveryPlan: "Review the CSV and repeat validation",
      stepBoundaries: ["automatable", "automatable", "automatable", "human-required"].map((disposition, index) => ({ stepId: `step-${String(index + 1).padStart(2, "0")}`, disposition: disposition as "automatable" | "human-required", rationale: "Documented boundary" })),
    };
    const blueprint = buildPreviewBlueprint(sourceIntake, draft, { observed: ["one observation"], declared: ["intake"], repeatedObservations: 1, observedDays: 1, declaredRepeatedDays: 1, rulesQuality: 1, observedWeeklyFrequency: 5 });
    expect(blueprint).toMatchObject({ status: "draft", completeness: { score: 100 } });
  });

  it("scores classified boundaries proportionally and never reports negative savings", () => {
    const sourceIntake = intake(["Export", "Resolve"], 30);
    sourceIntake.tools = ["Ledger"];
    const draft = { ...emptyBlueprintDraft(sourceIntake), runsPerWeek: 2, dataSensitivity: "moderate" as const, sideEffectLevel: "none" as const, verificationMode: "every-run" as const, verificationMinutesPerRun: 0, recoveryPlan: "Escalate unresolved rows", stepBoundaries: [{ stepId: "step-01", disposition: "automatable" as const, rationale: "Repeatable" }, { stepId: "step-02", disposition: "unassessed" as const }] };
    expect(computeCompleteness(draft, sourceFromIntake(sourceIntake)).score).toBe(90);
    expect(computeEstimates(draft, sourceFromIntake(sourceIntake), { observed: ["a", "b"], declared: ["intake"], repeatedObservations: 2, observedDays: 7, declaredRepeatedDays: 7, rulesQuality: 0.5, observedWeeklyFrequency: 2 })).toMatchObject({ weeklyBaselineMinutes: 60, weeklyResidualMinutes: 30, weeklySavingsMinutes: 30, effort: 34, operationalRisk: 19, confidence: 60 });

    const highVerification = { ...draft, runsPerWeek: 1, verificationMinutesPerRun: 20, stepBoundaries: [{ stepId: "step-01", disposition: "automatable" as const, rationale: "Repeatable" }] };
    const oneStepSource = sourceFromIntake(intake(["Export"], 10));
    expect(computeEstimates(highVerification, oneStepSource, { observed: [], declared: [], repeatedObservations: 0, observedDays: 0, declaredRepeatedDays: 0, rulesQuality: 0, observedWeeklyFrequency: 0 })).toMatchObject({ weeklyBaselineMinutes: 10, weeklyResidualMinutes: 20, weeklySavingsMinutes: 0 });
  });

  it("uses bounded risk and the evidence-quality confidence components", () => {
    const sourceIntake = intake(Array.from({ length: 11 }, (_, index) => `Step ${index + 1}`));
    sourceIntake.tools = Array.from({ length: 7 }, (_, index) => `Tool ${index + 1}`);
    const draft = { ...emptyBlueprintDraft(sourceIntake), runsPerWeek: 5, dataSensitivity: "high" as const, sideEffectLevel: "irreversible" as const, verificationMode: "none" as const };
    const capped = computeEstimates(draft, sourceFromIntake(sourceIntake), { observed: [], declared: [], repeatedObservations: 0, observedDays: 0, declaredRepeatedDays: 0, rulesQuality: 0, observedWeeklyFrequency: 0 });
    expect(capped).toMatchObject({ effort: 100, operationalRisk: 100 });

    const completeSource = intake();
    const completeDraft = { ...emptyBlueprintDraft(completeSource), runsPerWeek: 5, dataSensitivity: "low" as const, sideEffectLevel: "reversible" as const, verificationMode: "every-run" as const, verificationMinutesPerRun: 5, recoveryPlan: "Recover manually", stepBoundaries: ["automatable", "automatable", "automatable", "human-required"].map((disposition, index) => ({ stepId: `step-${String(index + 1).padStart(2, "0")}`, disposition: disposition as "automatable" | "human-required", rationale: "Documented" })) };
    expect(computeEstimates(completeDraft, sourceFromIntake(completeSource), { observed: ["one"], declared: ["intake"], repeatedObservations: 1, observedDays: 7, declaredRepeatedDays: 7, rulesQuality: 0.5, observedWeeklyFrequency: 2 })).toMatchObject({ confidence: 50 });
  });
});
