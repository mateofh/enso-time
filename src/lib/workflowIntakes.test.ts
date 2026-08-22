import { describe, expect, it } from "vitest";
import { candidateId } from "./candidateIdentity";
import { MAX_WORKFLOW_INTAKES, migrateWorkflowIntakes, normalizeWorkflowIntake, upsertWorkflowIntake } from "./workflowIntakes";
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
    const sharedId = candidateId("Preparar reporte semanal");
    const newest = normalizeWorkflowIntake({ ...intake(sharedId, "2026-08-21T11:00:00Z"), id: "workflow-original", trigger: "Versión nueva" }, NOW);
    const stale = { ...intake(sharedId, "2026-08-21T10:30:00Z"), id: "workflow-stale", trigger: "Versión obsoleta" };
    const staleResult = upsertWorkflowIntake([newest], stale, NOW);
    expect(staleResult.persisted.trigger).toBe("Versión nueva");
    expect(staleResult.items).toEqual([newest]);

    const replacement = { ...intake(sharedId, "2026-08-21T11:30:00Z"), id: "workflow-replacement", trigger: "Versión final" };
    const newestResult = upsertWorkflowIntake([newest], replacement, NOW);
    expect(newestResult.persisted).toMatchObject({ id: "workflow-original", trigger: "Versión final", createdAt: newest.createdAt });
  });

  it("migrates legacy candidate ids and deterministically collapses duplicates", () => {
    const legacySlug = { ...intake("candidate-preparar-reporte-semanal", "2026-08-21T11:00:00Z"), id: "workflow-z", trigger: "Más nuevo" };
    const legacyHash = { ...intake("candidate-49230d255d76d93f", "2026-08-21T10:00:00Z"), id: "workflow-a", trigger: "Más viejo" };
    const migrated = migrateWorkflowIntakes([legacyHash, legacySlug], NOW);
    expect(migrated).toHaveLength(1);
    expect(migrated[0]).toMatchObject({
      id: "workflow-z",
      candidateId: candidateId("Preparar reporte semanal"),
      candidateLabel: "Preparar reporte semanal",
      trigger: "Más nuevo",
    });
    expect(migrateWorkflowIntakes(migrated, NOW)).toEqual(migrated);

    const tiedA = { ...legacyHash, id: "workflow-a", updatedAt: legacySlug.updatedAt };
    const tiedB = { ...legacySlug, id: "workflow-b", trigger: "Empate B" };
    expect(migrateWorkflowIntakes([tiedB, tiedA], NOW)).toEqual(migrateWorkflowIntakes([tiedA, tiedB], NOW));
    expect(migrateWorkflowIntakes([tiedB, tiedA], NOW)[0]!.id).toBe("workflow-a");
  });

  it("enforces Rust minute and collection limits", () => {
    expect(() => normalizeWorkflowIntake({ ...intake(), minutesPerRun: 1_441 }, NOW)).toThrow("entre 1 y 1440");
    const full = Array.from({ length: MAX_WORKFLOW_INTAKES }, (_, index) => {
      const candidateLabel = `Reporte ${index}`;
      return { ...intake(candidateId(candidateLabel), "2026-08-21T10:00:00Z"), candidateLabel };
    });
    const overflowLabel = "Reporte overflow";
    expect(() => upsertWorkflowIntake(full, { ...intake(candidateId(overflowLabel)), candidateLabel: overflowLabel }, NOW)).toThrow("más de 500");
  });
});
