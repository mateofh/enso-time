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
  it("matches the complete four-step workflow vector", () => {
    const sourceIntake = intake();
    const draft = {
      ...emptyBlueprintDraft(sourceIntake), runsPerWeek: 5, dataSensitivity: "low" as const, sideEffectLevel: "reversible" as const, verificationMode: "every-run" as const, verificationMinutesPerRun: 5, recoveryPlan: "Review the CSV and repeat validation",
      stepBoundaries: ["automatable", "automatable", "automatable", "human-required"].map((disposition, index) => ({ stepId: `step-${String(index + 1).padStart(2, "0")}`, disposition: disposition as "automatable" | "human-required", rationale: "Documented boundary" })),
    };
    const blueprint = buildPreviewBlueprint(sourceIntake, draft, { observed: ["a", "b", "c", "d"], declared: ["intake"], repeatedObservations: 4 });
    expect(blueprint).toMatchObject({ status: "ready", completeness: { score: 100 }, estimates: { weeklyBaselineMinutes: 300, weeklyResidualMinutes: 100, weeklySavingsMinutes: 200, effort: 44, operationalRisk: 17, confidence: 89 } });
  });

  it("keeps a complete intake as a draft until a second observed repetition", () => {
    const sourceIntake = intake();
    const draft = {
      ...emptyBlueprintDraft(sourceIntake), runsPerWeek: 5, dataSensitivity: "low" as const, sideEffectLevel: "reversible" as const, verificationMode: "every-run" as const, verificationMinutesPerRun: 5, recoveryPlan: "Review the CSV and repeat validation",
      stepBoundaries: ["automatable", "automatable", "automatable", "human-required"].map((disposition, index) => ({ stepId: `step-${String(index + 1).padStart(2, "0")}`, disposition: disposition as "automatable" | "human-required", rationale: "Documented boundary" })),
    };
    const blueprint = buildPreviewBlueprint(sourceIntake, draft, { observed: ["one observation"], declared: ["intake"], repeatedObservations: 1 });
    expect(blueprint).toMatchObject({ status: "draft", completeness: { score: 100 } });
  });

  it("scores classified boundaries proportionally and never reports negative savings", () => {
    const sourceIntake = intake(["Export", "Resolve"], 30);
    const draft = { ...emptyBlueprintDraft(sourceIntake), runsPerWeek: 2, dataSensitivity: "moderate" as const, sideEffectLevel: "none" as const, verificationMode: "every-run" as const, verificationMinutesPerRun: 0, recoveryPlan: "Escalate unresolved rows", stepBoundaries: [{ stepId: "step-01", disposition: "automatable" as const, rationale: "Repeatable" }, { stepId: "step-02", disposition: "unassessed" as const }] };
    expect(computeCompleteness(draft, sourceFromIntake(sourceIntake)).score).toBe(90);
    expect(computeEstimates(draft, sourceFromIntake(sourceIntake), { observed: ["a", "b"], declared: ["intake"], repeatedObservations: 2 })).toMatchObject({ weeklyBaselineMinutes: 60, weeklyResidualMinutes: 30, weeklySavingsMinutes: 30, effort: 34, operationalRisk: 19, confidence: 60 });

    const highVerification = { ...draft, runsPerWeek: 1, verificationMinutesPerRun: 20, stepBoundaries: [{ stepId: "step-01", disposition: "automatable" as const, rationale: "Repeatable" }] };
    const oneStepSource = sourceFromIntake(intake(["Export"], 10));
    expect(computeEstimates(highVerification, oneStepSource, { observed: [], declared: [], repeatedObservations: 0 })).toMatchObject({ weeklyBaselineMinutes: 10, weeklyResidualMinutes: 20, weeklySavingsMinutes: 0 });
  });
});
