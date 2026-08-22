import { describe, expect, it } from "vitest";
import { MAX_WORKFLOW_INTAKES, normalizeWorkflowIntake, upsertWorkflowIntake } from "./workflowIntakes";
import type { WorkflowIntakeV1 } from "./types";

const NOW = new Date("2026-08-21T12:00:00Z");

function intake(candidateId = "candidate-report", updatedAt = "2026-08-21T10:00:00Z"): WorkflowIntakeV1 {
  return {
    id: `workflow-${candidateId}`,
    candidateId,
    candidateLabel: "  Preparar   reporte semanal ",
    trigger: " Cada viernes   por la tarde ",
    tools: [" Sheets ", "Email", "sheets"],
    steps: [" Exportar   datos ", " Revisar excepciones "],
    humanDecisions: " Revisar   excepciones ",
    inputs: " CSV   de ventas ",
    desiredOutput: " Reporte validado ",
    frequency: " Cada   viernes ",
    minutesPerRun: 45,
    painPoints: " Formatos   inconsistentes ",
    createdAt: "2026-08-20T10:00:00Z",
    updatedAt,
  };
}

describe("workflow intake preview parity", () => {
  it("normalizes text and deduplicates tools like Rust", () => {
    const normalized = normalizeWorkflowIntake(intake(), NOW);
    expect(normalized.candidateLabel).toBe("Preparar reporte semanal");
    expect(normalized.tools).toEqual(["Sheets", "Email"]);
    expect(normalized.steps).toEqual(["Exportar datos", "Revisar excepciones"]);
    expect(normalized.updatedAt).toBe("2026-08-21T10:00:00+00:00");
  });

  it("keeps the newest candidate update and returns the persisted record", () => {
    const newest = normalizeWorkflowIntake({ ...intake("candidate-report", "2026-08-21T11:00:00Z"), id: "workflow-original", candidateLabel: "Versión nueva" }, NOW);
    const stale = { ...intake("candidate-report", "2026-08-21T10:30:00Z"), id: "workflow-stale", candidateLabel: "Versión obsoleta" };
    const staleResult = upsertWorkflowIntake([newest], stale, NOW);
    expect(staleResult.persisted.candidateLabel).toBe("Versión nueva");
    expect(staleResult.items).toEqual([newest]);

    const replacement = { ...intake("candidate-report", "2026-08-21T11:30:00Z"), id: "workflow-replacement", candidateLabel: "Versión final" };
    const newestResult = upsertWorkflowIntake([newest], replacement, NOW);
    expect(newestResult.persisted).toMatchObject({ id: "workflow-original", candidateLabel: "Versión final", createdAt: newest.createdAt });
  });

  it("enforces Rust minute and collection limits", () => {
    expect(() => normalizeWorkflowIntake({ ...intake(), minutesPerRun: 1_441 }, NOW)).toThrow("entre 1 y 1440");
    const full = Array.from({ length: MAX_WORKFLOW_INTAKES }, (_, index) => intake(`candidate-${index}`, "2026-08-21T10:00:00Z"));
    expect(() => upsertWorkflowIntake(full, intake("candidate-overflow"), NOW)).toThrow("más de 500");
  });
});
