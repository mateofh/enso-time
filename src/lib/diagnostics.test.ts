import { describe, expect, it } from "vitest";
import { buildPrompt, candidateId, createDiagnostic, scoreCandidate } from "./diagnostics";
import type { CheckInV1, TimelineBlockV1 } from "./types";

const checkIns: CheckInV1[] = [1, 2, 3].map((day) => ({
  id: `c${day}`,
  intervalStart: `2026-08-0${day}T10:00:00Z`,
  intervalEnd: `2026-08-0${day}T10:15:00Z`,
  reason: "ambiguous",
  status: "answered",
  activityLabel: "Actualizar reporte",
  category: "admin",
  repetitive: true,
  rulesClear: true,
  createdAt: `2026-08-0${day}T10:15:00Z`,
}));

const blocks: TimelineBlockV1[] = [
  { id: "a", start: "2026-08-01T09:00:00Z", end: "2026-08-01T10:00:00Z", durationSeconds: 3600, app: "Code", category: "focus", confidence: 1, afk: false, source: "window" },
  { id: "b", start: "2026-08-01T10:00:00Z", end: "2026-08-01T10:30:00Z", durationSeconds: 1800, app: "Mail", category: "communication", confidence: 1, afk: false, source: "window" },
];

describe("automation diagnosis", () => {
  it("ranks repeated rule-based work", () => {
    const candidate = scoreCandidate(checkIns, 3)[0]!;
    expect(candidate.activityLabel).toBe("Actualizar reporte");
    expect(candidate.weeklyFrequency).toBe(5);
    expect(candidate.priorityScore).toBeGreaterThan(30);
  });

  it("builds an evidence-bound prompt", () => {
    const candidate = scoreCandidate(checkIns, 3)[0]!;
    const prompt = buildPrompt(candidate, "Operaciones", ["Reducir trabajo manual"], 3);
    expect(prompt).toContain("No inventes datos");
    expect(prompt).toContain("Actualizar reporte");
    expect(prompt).toContain("Operaciones");
  });

  it("marks studies under seven days as preliminary", () => {
    const diagnostic = createDiagnostic(blocks, checkIns, 3);
    expect(diagnostic.preliminary).toBe(true);
    expect(diagnostic.metrics.find((metric) => metric.label === "Tiempo activo")?.value).toBe(5400);
  });

  it("matches the shared FNV-1a candidate id for Unicode and emoji", () => {
    expect(candidateId("  Café semanal  ")).toBe("candidate-0e357a784791391c");
    expect(candidateId("🤖 Reporte 📊")).toBe("candidate-98d85f5650810211");
  });

  it("breaks equal-priority ties by candidate id", () => {
    const tied = ["Zulu", "Árbol"].flatMap((activityLabel, group) => [1, 2].map((day) => ({
      ...checkIns[0]!,
      id: `tie-${group}-${day}`,
      activityLabel,
      intervalStart: `2026-08-0${day}T10:00:00Z`,
      intervalEnd: `2026-08-0${day}T10:15:00Z`,
    })));
    const candidates = scoreCandidate(tied, 2);
    expect(candidates).toHaveLength(2);
    expect(candidates[0]!.priorityScore).toBe(candidates[1]!.priorityScore);
    expect(candidates.map((candidate) => candidate.id)).toEqual([...candidates.map((candidate) => candidate.id)].sort());
  });
});
