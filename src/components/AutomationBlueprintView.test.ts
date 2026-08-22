import { describe, expect, it } from "vitest";
import { errorCode, resolveBlueprintCommandResult } from "./AutomationBlueprintView";
import { buildPreviewBlueprint } from "../lib/blueprints";
import type { WorkflowIntakeV1 } from "../lib/types";

const intake: WorkflowIntakeV1 = {
  id: "workflow-view", candidateId: "candidate-view", candidateLabel: "Weekly report", trigger: "Close", tools: ["Sheet"], steps: ["Export"], humanDecisions: "Review", inputs: "CSV", desiredOutput: "Report", frequency: "Weekly", minutesPerRun: 30, painPoints: "Formatting", createdAt: "2026-08-20T12:00:00.000Z", updatedAt: "2026-08-21T12:00:00.000Z",
};

describe("automation blueprint command outcomes", () => {
  it("localizes native Err(String) and object-shaped Rust errors", () => {
    expect(errorCode('Err("blueprint.conflict")')).toBe("blueprint.conflict");
    expect(errorCode(new Error('Err("blueprint.conflict")'))).toBe("blueprint.conflict");
    expect(errorCode({ Err: "blueprint.stale" })).toBe("blueprint.stale");
    expect(errorCode({ message: "blueprint.not_ready" })).toBe("blueprint.not_ready");
  });

  it("keeps the current response and suppresses success when an Ok result contains a conflict", () => {
    const current = { ...buildPreviewBlueprint(intake), conflict: { currentRevision: 4 } };
    expect(resolveBlueprintCommandResult(current, "Saved")).toEqual({ blueprint: current, draft: current.draft, error: "blueprint.conflict", notice: undefined });
  });
});
