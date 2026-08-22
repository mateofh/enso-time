import { describe, expect, it } from "vitest";
import { previewBlueprintEvidence } from "./bridge";
import type { CheckInV1, WorkflowIntakeV1 } from "./types";

const intake: WorkflowIntakeV1 = {
  id: "workflow-evidence",
  candidateId: "candidate-evidence",
  candidateLabel: "Weekly report",
  trigger: "Close",
  tools: ["Sheet"],
  steps: ["Export"],
  humanDecisions: "Review",
  inputs: "CSV",
  desiredOutput: "Report",
  frequency: "Weekly",
  minutesPerRun: 30,
  painPoints: "Formatting",
  createdAt: "2026-08-20T12:00:00.000Z",
  updatedAt: "2026-08-21T12:00:00.000Z",
};

function answered(id: string, intervalStart: string, repetitive: boolean, rulesClear: boolean): CheckInV1 {
  return {
    id,
    intervalStart,
    intervalEnd: "2026-08-04T12:15:00.000Z",
    reason: "ambiguous",
    status: "answered",
    activityLabel: "  WEEKLY REPORT  ",
    category: "admin",
    repetitive,
    rulesClear,
    createdAt: "2026-08-04T12:15:00.000Z",
    answeredAt: "2026-08-04T12:16:00.000Z",
  };
}

describe("preview blueprint evidence", () => {
  it("keeps recorded intervals distinct from repetition confirmations", () => {
    const evidence = previewBlueprintEvidence(intake, [
      answered("checkin-01", "2026-08-01T12:00:00.000Z", true, true),
      answered("checkin-02", "2026-08-02T12:00:00.000Z", false, true),
      answered("checkin-03", "2026-08-03T12:00:00.000Z", true, false),
      { ...answered("checkin-other", "2026-08-04T12:00:00.000Z", true, true), activityLabel: "Different workflow" },
      { ...answered("checkin-pending", "2026-08-04T12:00:00.000Z", true, true), status: "pending" },
    ], "en");

    expect(evidence).toMatchObject({
      repeatedObservations: 2,
      observedDays: 3,
      declaredRepeatedDays: 2,
      rulesQuality: 0.5,
      observedWeeklyFrequency: 7,
    });
    expect(evidence.observed).toHaveLength(3);
    expect(evidence.observed.every((item) => item.startsWith("Recorded check-in interval:"))).toBe(true);
    expect(evidence.observed.join(" ")).not.toContain("confirmed");
    expect(evidence.declared).toContain("Activity label confirmed by you:   WEEKLY REPORT  ");
    expect(evidence.declared).toContain("Repetition frequency confirmed by you: 2 confirmation(s) across 2 declared UTC day(s)");

    const spanishEvidence = previewBlueprintEvidence(intake, [
      answered("checkin-01", "2026-08-01T12:00:00.000Z", true, true),
      answered("checkin-02", "2026-08-02T12:00:00.000Z", false, true),
      answered("checkin-03", "2026-08-03T12:00:00.000Z", true, false),
    ], "es-419");
    expect(spanishEvidence.observed).toHaveLength(3);
    expect(spanishEvidence.declared).toContain("Frecuencia de repetición confirmada por ti: 2 confirmación(es) en 2 día(s) UTC declarados");
  });
});
