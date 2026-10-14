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

  it("renders English diagnosis without translating user content", () => {
    const diagnostic = createDiagnostic(blocks, checkIns, 3, "Operaciones", ["Reducir trabajo manual"], "en");
    expect(diagnostic.metrics[0]?.label).toBe("Active time");
    expect(diagnostic.prompt).toContain("TOP CANDIDATE");
    expect(diagnostic.prompt).toContain("Actualizar reporte");
    expect(diagnostic.prompt).toContain("Reducir trabajo manual");
    expect(diagnostic.candidates[0]?.evidence[0]).toBe("3 sessions: 0 corrections, 0 rules and 3 check-ins");
    expect(diagnostic.recommendations.join(" ")).not.toContain("Valida “");
  });

  it("uses recorded durations instead of assuming every interval is 15 minutes", () => {
    const longer = checkIns.map((item) => ({ ...item, intervalEnd: item.intervalEnd.replace("10:15", "10:30") }));
    expect(scoreCandidate(longer, 3)[0]?.estimatedWeeklyHours).toBe(2.5);
  });

  it("marks studies under seven days as preliminary", () => {
    const diagnostic = createDiagnostic(blocks, checkIns, 3);
    expect(diagnostic.preliminary).toBe(true);
    expect(diagnostic.metrics.find((metric) => metric.label === "Tiempo activo")?.value).toBe(5400);
  });

  it("matches the shared FNV-1a candidate id for Unicode and emoji", () => {
    const vectors = [
      ["  Café semanal  ", "candidate-f329e559e8fa0e7c"],
      ["🤖 Reporte 📊", "candidate-b5d6eca88706b271"],
      ["hello", "candidate-a430d84680aabd0b"],
      ["ÁRBOL", "candidate-69684103a78f978a"],
      ["Conciliación 💸", "candidate-311f7ffd34d13298"],
      ["Enviar 🚀", "candidate-c46550dd53ba9a5f"],
      ["日本語💡", "candidate-e1b1c5d6a1fc81ff"],
      ["PREPARAR reporte 🚀", "candidate-dd9d3a169690844a"],
    ] as const;
    for (const [label, expected] of vectors) expect(candidateId(label)).toBe(expected);
    expect(candidateId(" Preparar ")).toBe(candidateId("preparar"));
    expect(candidateId("ÁRBOL")).toBe(candidateId("Árbol"));
    expect(candidateId("Árbol")).not.toBe(candidateId("árbol"));
  });

  it("groups ASCII casing and chooses the bytewise-smallest original label", () => {
    const mixedCase = ["preparar", "Preparar", "PREPARAR"].map((activityLabel, index) => ({
      ...checkIns[0]!,
      id: `mixed-${index}`,
      activityLabel,
    }));
    for (const values of [mixedCase, [...mixedCase].reverse()]) {
      const [candidate] = scoreCandidate(values, 3);
      expect(candidate).toMatchObject({
        id: "candidate-7ef2468d3a5dcd66",
        activityLabel: "PREPARAR",
        weeklyFrequency: 5,
      });
    }
  });

  it("breaks equal-priority ties by candidate id", () => {
    const tied = ["Zulu", "Árbol"].flatMap((activityLabel, group) => [1, 2].map((day) => ({
      ...checkIns[0]!,
      id: `tie-${group}-${day}`,
      activityLabel,
      intervalStart: `2026-08-0${day}T10:00:00Z`,
      intervalEnd: `2026-08-0${day}T10:15:00Z`,
    })));
    const expectedIds = scoreCandidate(tied, 2).map((candidate) => candidate.id);
    for (let offset = 0; offset < tied.length; offset += 1) {
      const rotated = [...tied.slice(offset), ...tied.slice(0, offset)];
      const candidates = scoreCandidate(rotated, 2);
      expect(candidates).toHaveLength(2);
      expect(candidates[0]!.priorityScore).toBe(candidates[1]!.priorityScore);
      expect(candidates.map((candidate) => candidate.id)).toEqual(expectedIds);
    }
    expect(expectedIds).toEqual([...expectedIds].sort());
  });
});
