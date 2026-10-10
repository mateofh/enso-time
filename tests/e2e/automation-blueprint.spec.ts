import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";

const vectors = JSON.parse(readFileSync(new URL("../fixtures/automation-blueprint-vectors.json", import.meta.url), "utf8")) as {
  schemaVersion: number;
  vectors: Array<{ name: string; expected: Record<string, number | string>; checkIns?: Array<Record<string, unknown>> }>;
  edgeCases: { oversizeDraftBytes: number };
};

/**
 * v0.2 UI contract. The blueprint calls below deliberately run through the
 * same `window.__TAURI_INTERNALS__.invoke` path as desktop, rather than
 * preview-only localStorage flags. Keep these ids stable:
 * blueprint-entry, blueprint-panel, blueprint-status, blueprint-completeness,
 * blueprint-source-map, blueprint-boundary, blueprint-copy, blueprint-export,
 * blueprint-refresh, blueprint-delete-preview, blueprint-error,
 * blueprint-conflict, blueprint-loading, blueprint-evidence-count.
 */
const candidateId = "candidate-d9a1c758c688afa3";
const Spanish = {
  trigger: "Llega el cierre semanal de ventas",
  input: "CSV de ventas y tabla de metas",
  output: "Reporte validado y listo para compartir",
  tool: "Hoja de cálculo local",
  step1: "Exportar las oportunidades cerradas",
  step2: "Validar montos y responsables",
  human: "Excluir cuentas con disputas abiertas",
};

const intake = {
  id: "workflow-e2e-blueprint",
  candidateId,
  candidateLabel: "Actualizar reporte semanal",
  trigger: Spanish.trigger,
  tools: [Spanish.tool, "Mensajería interna"],
  steps: [Spanish.step1, Spanish.step2],
  humanDecisions: Spanish.human,
  inputs: Spanish.input,
  desiredOutput: Spanish.output,
  frequency: "Cada viernes",
  minutesPerRun: 45,
  painPoints: "Corregir formatos y buscar datos faltantes",
  createdAt: "2026-08-20T12:00:00.000Z",
  updatedAt: "2026-08-21T12:00:00.000Z",
};

type GetMode = "success" | "pending" | "reject" | "native-err";
type CommandMode = "success" | "reject" | "conflict-ok";
type DraftSeed = { revision?: number; sourceIntakeUpdatedAt?: string; complete?: boolean };

function checkIns(count: number) {
  return Array.from({ length: count }, (_, index) => {
    const day = String(index + 1).padStart(2, "0");
    return {
      id: `e2e-blueprint-checkin-${index + 1}`,
      intervalStart: `2026-08-${day}T15:00:00.000Z`,
      intervalEnd: `2026-08-${day}T15:15:00.000Z`,
      reason: "ambiguous",
      status: "answered",
      activityLabel: intake.candidateLabel,
      category: "admin",
      project: "Operación",
      repetitive: true,
      rulesClear: true,
      createdAt: `2026-08-${day}T15:15:00.000Z`,
      answeredAt: `2026-08-${day}T15:16:00.000Z`,
    };
  });
}

type CheckInSeed = ReturnType<typeof checkIns>[number];

/**
 * A narrow desktop backend double. It supplies the app bootstrap commands so
 * setting __TAURI_INTERNALS__ cannot accidentally fall back to preview state,
 * while assertions remain focused on the public v0.2 invoke contract.
 */
async function seedDesktop(page: Page, options: {
  locale?: "es-419" | "en";
  observations?: number;
  checkIns?: CheckInSeed[];
  intakes?: typeof intake[];
  draft?: DraftSeed;
  getMode?: GetMode;
  saveMode?: CommandMode;
  refreshMode?: CommandMode;
  currentRevision?: number;
} = {}) {
  const locale = options.locale ?? "es-419";
  await page.addInitScript((scenario) => {
    const selectedIntake = scenario.intakes[0];
    const state = {
      getMode: scenario.getMode,
      saveMode: scenario.saveMode,
      refreshMode: scenario.refreshMode,
      currentRevision: scenario.currentRevision,
      calls: [] as Array<{ command: string; args: Record<string, unknown> }>,
    };
    const source = selectedIntake && {
      trigger: selectedIntake.trigger,
      inputs: selectedIntake.inputs,
      desiredOutput: selectedIntake.desiredOutput,
      tools: selectedIntake.tools,
      steps: selectedIntake.steps.map((label: string, index: number) => ({ id: `step-${String(index + 1).padStart(2, "0")}`, label })),
      humanDecisions: selectedIntake.humanDecisions,
      minutesPerRun: selectedIntake.minutesPerRun,
      frequency: selectedIntake.frequency,
      painPoints: selectedIntake.painPoints,
    };
    const matchingCheckIns = scenario.checkIns.filter((item: { status: string; activityLabel: string }) =>
      item.status === "answered" && item.activityLabel === selectedIntake?.candidateLabel,
    );
    const repeatedCheckIns = matchingCheckIns.filter((item: { repetitive: boolean }) => item.repetitive);
    const observedDays = new Set(matchingCheckIns.map((item: { intervalStart: string }) => item.intervalStart.slice(0, 10))).size;
    const declaredRepeatedDays = new Set(repeatedCheckIns.map((item: { intervalStart: string }) => item.intervalStart.slice(0, 10))).size;
    const rulesQuality = repeatedCheckIns.length ? repeatedCheckIns.filter((item: { rulesClear: boolean }) => item.rulesClear).length / repeatedCheckIns.length : 0;
    const evidence = {
      observed: matchingCheckIns.map((item: { intervalStart: string; intervalEnd: string }) => `Intervalo de check-in registrado: ${item.intervalStart}–${item.intervalEnd}`),
      declared: [
        "Intake del workflow declarado por ti",
        ...(repeatedCheckIns.length ? [`Frecuencia de repetición confirmada por ti: ${repeatedCheckIns.length} confirmación(es) en ${declaredRepeatedDays} día(s) UTC declarados`] : []),
      ],
      repeatedObservations: repeatedCheckIns.length,
      observedDays,
      declaredRepeatedDays,
      rulesQuality,
      observedWeeklyFrequency: declaredRepeatedDays ? repeatedCheckIns.length * 7 / declaredRepeatedDays : 0,
    };
    const emptyDraft = () => ({
      schemaVersion: 1,
      id: `blueprint-${selectedIntake.candidateId}`,
      candidateId: selectedIntake.candidateId,
      workflowIntakeId: selectedIntake.id,
      sourceIntakeUpdatedAt: scenario.draft.sourceIntakeUpdatedAt ?? selectedIntake.updatedAt,
      revision: scenario.draft.revision ?? 0,
      runsPerWeek: scenario.draft.complete ? 5 : undefined,
      stepBoundaries: source.steps.map((step: { id: string }) => ({
        stepId: step.id,
        disposition: scenario.draft.complete ? "automatable" : "unassessed",
        rationale: scenario.draft.complete ? "Reglas repetibles y reversibles." : undefined,
      })),
      dataSensitivity: scenario.draft.complete ? "low" : "unassessed",
      sideEffectLevel: scenario.draft.complete ? "reversible" : "unassessed",
      verificationMode: scenario.draft.complete ? "every-run" : "unassessed",
      verificationMinutesPerRun: scenario.draft.complete ? 5 : undefined,
      recoveryPlan: scenario.draft.complete ? "Revisar el CSV y repetir la validación manual." : undefined,
      createdAt: "2026-08-20T12:00:00.000Z",
      updatedAt: "2026-08-21T12:00:00.000Z",
    });
    const isComplete = (draft: ReturnType<typeof emptyDraft>) => Boolean(
      draft.runsPerWeek
      && draft.dataSensitivity !== "unassessed"
      && draft.sideEffectLevel !== "unassessed"
      && draft.verificationMode !== "unassessed"
      && draft.verificationMinutesPerRun !== undefined
      && draft.recoveryPlan
      && draft.stepBoundaries.every((item) => item.disposition !== "unassessed" && item.rationale),
    );
    const buildBlueprint = (draft: ReturnType<typeof emptyDraft>, conflict?: { currentRevision: number }) => {
      const complete = isComplete(draft);
      // The status is derived from current intake data and evidence; tests never
      // seed a status field. This catches stale snapshots and weak evidence.
      const status = draft.sourceIntakeUpdatedAt !== selectedIntake.updatedAt
        ? "stale"
        : !source.steps.length
          ? "blocked"
          : complete && evidence.repeatedObservations >= 2 && draft.stepBoundaries.some((item) => item.disposition === "automatable")
            ? "ready"
            : "draft";
      return {
        schemaVersion: 1,
        draft,
        source,
        evidence,
        status,
        completeness: { score: complete ? 100 : 35, missing: complete ? [] : ["runsPerWeek", "stepBoundaries", "recoveryPlan"] },
        estimates: (() => {
          const frequencyAgreement = Math.max(0, 1 - Math.abs(evidence.observedWeeklyFrequency - (draft.runsPerWeek ?? 0)) / Math.max(1, draft.runsPerWeek ?? 0));
          const confidence = Math.round(Math.min(30, 6 * evidence.repeatedObservations) + Math.round(20 * Math.min(evidence.observedDays, 14) / 14) + Math.round(20 * evidence.rulesQuality) + Math.round(10 * frequencyAgreement) + Math.round(20 * (complete ? 1 : 0.35)));
          return { weeklyBaselineMinutes: 225, weeklyResidualMinutes: 118, weeklySavingsMinutes: 107, effort: 34, operationalRisk: 17, confidence, components: { observations: evidence.repeatedObservations, observedDays: evidence.observedDays, declaredRepeatedDays: evidence.declaredRepeatedDays, rulesQuality: evidence.rulesQuality, observedWeeklyFrequency: evidence.observedWeeklyFrequency, frequencyAgreement } };
        })(),
        renderedMarkdown: `# Automation blueprint\n\nStatus: ${status}`,
        ...(conflict ? { conflict } : {}),
      };
    };
    const conflictResult = (draft: ReturnType<typeof emptyDraft>) => buildBlueprint({
      ...draft,
      revision: state.currentRevision,
      sourceIntakeUpdatedAt: selectedIntake.updatedAt,
      runsPerWeek: 9,
      recoveryPlan: "Revisar la versión actual antes de continuar.",
    }, { currentRevision: state.currentRevision });
    let currentDraft = selectedIntake ? emptyDraft() : undefined;
    const testApi = {
      state,
      setGetMode(mode: GetMode) { state.getMode = mode; },
      calls(command: string) { return state.calls.filter((call) => call.command === command); },
      blueprint() { return buildBlueprint(currentDraft!); },
    };
    (window as Window & { __blueprintE2E?: typeof testApi }).__blueprintE2E = testApi;
    (window as Window & { __TAURI_INTERNALS__?: { invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown> } }).__TAURI_INTERNALS__ = {
      async invoke(command, args = {}) {
        state.calls.push({ command, args });
        switch (command) {
          case "get_settings": return scenario.settings;
          case "get_tracking_status": return { running: true, windowWatcher: "running", afkWatcher: "running", pendingCheckIns: 0, dataStaysLocal: true, processingMode: "local-only" };
          case "get_timeline": return [];
          case "list_classification_rules": return [];
          case "list_checkins": return scenario.checkIns;
          case "get_study_progress": return { startedAt: "2026-08-20T12:00:00.000Z", selectedDays: 7, observedDays: 2, trackedHours: 1, answeredCheckIns: scenario.checkIns.length, coveragePercent: 100 };
          case "list_workflow_intakes": return scenario.intakes;
          case "get_ai_configuration_status": return { hasApiKey: false, ready: false, consentCurrent: false, spentThisMonthUsd: 0 };
          case "list_ai_reports": return [];
          case "generate_diagnostic": return { schemaVersion: 1, rangeStart: "2026-08-20T00:00:00.000Z", rangeEnd: "2026-08-21T00:00:00.000Z", daysObserved: 2, metrics: [], recommendations: [], candidates: [], prompt: "", preliminary: false, generatedAt: "2026-08-21T00:00:00.000Z" };
          case "get_automation_blueprint_v1":
            if (state.getMode === "pending") return new Promise<never>(() => undefined);
            if (state.getMode === "reject") throw new Error("backend unavailable");
            if (state.getMode === "native-err") throw 'Err("blueprint.not_ready")';
            return buildBlueprint(currentDraft!);
          case "save_automation_blueprint_v1":
            if (state.saveMode === "reject") throw new Error("save unavailable");
            const input = args.input as { draft: ReturnType<typeof emptyDraft>; expectedRevision?: number };
            currentDraft = { ...input.draft, revision: (input.expectedRevision ?? input.draft.revision) + 1, updatedAt: "2026-08-22T12:00:00.000Z" };
            if (state.saveMode === "conflict-ok") return conflictResult(currentDraft);
            return buildBlueprint(currentDraft);
          case "refresh_automation_blueprint_source_v1":
            currentDraft = { ...currentDraft!, sourceIntakeUpdatedAt: selectedIntake.updatedAt, revision: currentDraft!.revision + 1 };
            if (state.refreshMode === "reject") throw new Error("refresh unavailable");
            if (state.refreshMode === "conflict-ok") return conflictResult(currentDraft);
            return buildBlueprint(currentDraft);
          case "export_automation_blueprint_v1": return `desktop://automation-blueprint-${selectedIntake.candidateId}.md`;
          case "delete_automation_blueprint_v1": return undefined;
          default: throw new Error(`Unexpected Tauri command: ${command}`);
        }
      },
    };
  }, {
    getMode: options.getMode ?? "success",
    saveMode: options.saveMode ?? "success",
    refreshMode: options.refreshMode ?? "success",
    currentRevision: options.currentRevision ?? 4,
    draft: options.draft ?? {},
    intakes: options.intakes ?? [intake],
    checkIns: options.checkIns ?? checkIns(options.observations ?? 2),
    settings: { schemaVersion: 2, onboardingCompleted: true, locale, theme: "ink", timezone: "America/Bogota", cadenceMinutes: 15, autostart: true, captureWindowTitles: true, retentionDays: 90, updateChecksEnabled: false, goals: ["Recuperar tiempo"], role: "Operaciones", workSchedule: { start: "08:30", end: "18:00", days: [1, 2, 3, 4, 5] }, appRules: [], deepCapture: { browser: { enabled: false, cloudAllowed: false }, editor: { enabled: false, cloudAllowed: false }, shell: { enabled: false, cloudAllowed: false }, screenshots: { enabled: false, cloudAllowed: false }, screenshotIntervalMinutes: 15, browserTextAllowlist: [], excludedApps: ["1Password"], excludedSites: [] }, ai: { enabled: false, weeklyEnabled: false, weeklyDay: 1, weeklyHour: 8 } },
  });
  await page.goto("/");
}

async function openBlueprint(page: Page) {
  await page.getByRole("button", { name: /Automatizar|Automate/ }).click();
  await page.getByTestId("blueprint-entry").click();
}

async function completeDraft(page: Page) {
  await page.getByLabel(/Frecuencia semanal|Runs per week/i).fill("5");
  await page.getByLabel(/Minutos de verificación|Verification minutes/i).fill("5");
  await page.getByLabel(/Sensibilidad de datos|Data sensitivity/i).selectOption("low");
  await page.getByLabel(/Efectos secundarios|Side effects/i).selectOption("reversible");
  await page.getByLabel(/Modo de verificación|Verification mode/i).selectOption("every-run");
  await page.getByLabel(/Plan de recuperación|Recovery plan/i).fill("Revisar el CSV y repetir la validación manual.");
  const boundaries = page.getByTestId("blueprint-boundary");
  await expect(boundaries).toHaveCount(2);
  for (const [index, disposition] of ["automatable", "human-required"].entries()) {
    const boundary = boundaries.nth(index);
    await boundary.getByLabel(/Clasificación|Disposition/i).selectOption(disposition);
    await boundary.getByLabel(/Racional|Rationale/i).fill(index === 0 ? "Reglas repetibles y reversibles." : "Requiere resolver excepciones.");
  }
}

function invokeCalls(page: Page, command: string) {
  return page.evaluate((name) => window.__blueprintE2E?.calls(name), command);
}

function currentBlueprint(page: Page) {
  return page.evaluate(() => window.__blueprintE2E?.blueprint());
}

test.describe("Automation Blueprint v0.2 contract", () => {
  test("ES complete desktop journey sends the v0.2 save and export contracts", async ({ page }) => {
    await seedDesktop(page);
    await openBlueprint(page);
    await expect(page.getByTestId("blueprint-panel")).toBeVisible();
    await expect(page.getByTestId("blueprint-status")).toHaveText(/borrador|draft/i);
    await expect(page.getByTestId("blueprint-source-map")).toContainText(Spanish.trigger);
    await expect(page.getByTestId("blueprint-source-map")).toContainText(Spanish.output);
    await expect(page.getByTestId("blueprint-evidence-count")).toHaveText("2");
    await expect(page.getByTestId("blueprint-copy")).toBeDisabled();
    await expect(page.getByTestId("blueprint-export")).toBeDisabled();

    await completeDraft(page);
    await page.getByTestId("blueprint-save").click();
    await expect(page.getByTestId("blueprint-status")).toHaveText(/listo|ready/i);
    await expect(page.getByTestId("blueprint-completeness")).toHaveText(/100/);
    await expect(page.getByTestId("blueprint-copy")).toBeEnabled();
    await expect(page.getByTestId("blueprint-export")).toBeEnabled();
    await expect(page.getByTestId("blueprint-assumptions")).toContainText(/mismo tiempo|equal[- ]time/i);

    const [save] = await invokeCalls(page, "save_automation_blueprint_v1") ?? [];
    expect(save.args).toMatchObject({ locale: "es-419", input: { expectedRevision: 0, draft: { id: `blueprint-${candidateId}`, candidateId, sourceIntakeUpdatedAt: intake.updatedAt } } });
    await page.getByTestId("blueprint-export").click();
    await expect(page.getByRole("status")).toContainText(/exportado|exported/i);
    const [exportCall] = await invokeCalls(page, "export_automation_blueprint_v1") ?? [];
    expect(exportCall.args).toEqual({ candidateId, expectedRevision: 1, format: "markdown", locale: "es-419" });
  });

  test("English changes chrome only and preserves Spanish intake content", async ({ page }) => {
    await seedDesktop(page, { locale: "en" });
    await openBlueprint(page);
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
    await expect(page.getByTestId("blueprint-source-map")).toContainText(Spanish.trigger);
    await expect(page.getByTestId("blueprint-source-map")).toContainText(Spanish.human);
    await expect(page.locator('a[href^="http://"], a[href^="https://"]')).toHaveCount(0);
    await expect(page.locator('input[name*="oauth" i], input[name*="token" i], input[name*="secret" i], input[name*="credential" i]')).toHaveCount(0);
  });

  test("ready evidence separates recorded intervals from signals confirmed by the person", async ({ page }) => {
    await seedDesktop(page, { draft: { complete: true } });
    await openBlueprint(page);
    const evidence = page.getByTestId("blueprint-source-map");
    const panel = page.getByTestId("blueprint-panel");
    await expect(page.getByTestId("blueprint-status")).toContainText(/respaldado.*confirmadas|backed.*confirmed/i);
    await expect(evidence).toContainText(/intervalos.*registrados|recorded intervals/i);
    await expect(evidence).toContainText(/confirmado por ti|confirmed by you/i);
    await expect(panel).toContainText(/intervalos\/check-ins registrados son hechos de solo lectura|recorded intervals\/check-ins are read-only facts/i);
    await expect(panel).toContainText(/etiqueta, repetición, reglas y frecuencia son confirmaciones declaradas por ti|activity label, repetition, rules, and frequency are confirmations you declared/i);
  });

  test("production evidence vector derives recorded and declared days independently", async ({ page }) => {
    const vector = vectors.vectors.find((item) => item.name === "three recorded check-ins with two declared repetitions");
    expect(vector?.checkIns).toHaveLength(3);
    await seedDesktop(page, { draft: { complete: true }, checkIns: vector!.checkIns! as unknown as CheckInSeed[] });
    await openBlueprint(page);

    const blueprint = await currentBlueprint(page);
    expect(blueprint).toMatchObject({
      status: vector!.expected.status,
      evidence: {
        observedDays: vector!.expected.observedDays,
        declaredRepeatedDays: vector!.expected.declaredRepeatedDays,
        repeatedObservations: vector!.expected.repeatedObservations,
        observedWeeklyFrequency: vector!.expected.observedWeeklyFrequency,
      },
      estimates: { confidence: vector!.expected.confidence },
    });

    const columns = page.getByTestId("blueprint-source-map").locator(".evidence-columns > div");
    const recorded = columns.nth(0);
    const declared = columns.nth(1);
    await expect(page.getByTestId("blueprint-evidence-count")).toHaveText("2");
    await expect(recorded).toContainText(/intervalo de check-in registrado/i);
    await expect(recorded).not.toContainText(/confirmad|repetici[oó]n|reglas/i);
    await expect(declared).toContainText(/frecuencia de repetición confirmada por ti: 2 confirmación\(es\) en 2 día\(s\) UTC declarados/i);
    await expect(declared).not.toContainText(/intervalo de check-in registrado/i);
    await expect(page.locator(".blueprint-estimates")).toContainText(/52\/100.*confianza/i);
  });

  test("mobile and keyboard users can complete boundaries with visible focus", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await seedDesktop(page);
    await openBlueprint(page);
    await expect(page.getByTestId("blueprint-panel")).toBeVisible();
    await page.getByTestId("blueprint-save").focus();
    await expect(page.getByTestId("blueprint-save")).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(page.locator(":focus-visible")).toHaveCount(1);
    await page.keyboard.press("Tab");
    await expect(page.locator(":focus-visible")).toHaveCount(1);
    await expect(page.getByTestId("blueprint-panel")).toHaveCSS("overflow", /visible|auto|scroll/);
  });

  test("a stale snapshot is derived from a newer intake and refreshes through Tauri", async ({ page }) => {
    await seedDesktop(page, { draft: { revision: 3, complete: true, sourceIntakeUpdatedAt: "2026-08-20T12:00:00.000Z" } });
    await openBlueprint(page);
    await expect(page.getByTestId("blueprint-status")).toContainText(/desactualizad[oa]|stale/i);
    await expect(page.getByTestId("blueprint-refresh")).toBeEnabled();
    await expect(page.getByTestId("blueprint-copy")).toBeDisabled();
    await expect(page.getByTestId("blueprint-export")).toBeDisabled();
    await page.getByTestId("blueprint-refresh").click();
    await expect(page.getByTestId("blueprint-refresh-confirmation")).toBeVisible();
    const [refresh] = await invokeCalls(page, "refresh_automation_blueprint_source_v1") ?? [];
    expect(refresh.args).toEqual({ candidateId, expectedRevision: 3, locale: "es-419" });
  });

  test("pending and rejected get requests expose loading, error, and retry", async ({ page }) => {
    await seedDesktop(page, { getMode: "pending" });
    await openBlueprint(page);
    await expect(page.getByTestId("blueprint-loading")).toBeVisible();

    await seedDesktop(page, { getMode: "reject" });
    await openBlueprint(page);
    await expect(page.getByTestId("blueprint-error")).toContainText(/backend|disponible|unavailable/i);
    await page.evaluate(() => window.__blueprintE2E?.setGetMode("success"));
    await page.getByTestId("blueprint-retry").click();
    await expect(page.getByTestId("blueprint-panel")).toBeVisible();
  });

  test("native Rust Err(String) is localized without leaking its transport wrapper", async ({ page }) => {
    await seedDesktop(page, { getMode: "native-err" });
    await openBlueprint(page);
    await expect(page.getByTestId("blueprint-error")).toContainText(/completa.*blueprint|complete.*blueprint/i);
    await expect(page.getByTestId("blueprint-error")).not.toContainText(/Err\(/);
  });

  test("save rejection is explicit", async ({ page }) => {
    await seedDesktop(page, { draft: { complete: true, revision: 3 }, saveMode: "reject" });
    await openBlueprint(page);
    await page.getByTestId("blueprint-save").click();
    await expect(page.getByRole("alert")).toContainText(/save|guardar|unavailable/i);
    const [save] = await invokeCalls(page, "save_automation_blueprint_v1") ?? [];
    expect(save.args).toMatchObject({ locale: "es-419", input: { expectedRevision: 3 } });
  });

  test("save and refresh Ok conflicts keep the current draft and suppress success", async ({ page }) => {
    await seedDesktop(page, { draft: { complete: true, revision: 3 }, saveMode: "conflict-ok", currentRevision: 4 });
    await openBlueprint(page);
    await page.getByTestId("blueprint-save").click();
    await expect(page.getByTestId("blueprint-conflict")).toContainText(/conflicto|conflict/i);
    await expect(page.getByTestId("blueprint-conflict")).toContainText("4");
    await expect(page.getByTestId("blueprint-save")).toBeDisabled();
    await expect(page.getByLabel(/Frecuencia semanal|Runs per week/i)).toHaveValue("9");
    await expect(page.getByLabel(/Plan de recuperación|Recovery plan/i)).toHaveValue("Revisar la versión actual antes de continuar.");
    await expect(page.getByRole("status")).toHaveCount(0);

    await seedDesktop(page, { draft: { complete: true, revision: 3, sourceIntakeUpdatedAt: "2026-08-20T12:00:00.000Z" }, refreshMode: "conflict-ok", currentRevision: 4 });
    await openBlueprint(page);
    await page.getByTestId("blueprint-refresh").click();
    await expect(page.getByTestId("blueprint-conflict")).toContainText(/conflicto|conflict/i);
    await expect(page.getByTestId("blueprint-conflict")).toContainText("4");
    await expect(page.getByTestId("blueprint-refresh-confirmation")).toHaveCount(0);
    await expect(page.getByLabel(/Frecuencia semanal|Runs per week/i)).toHaveValue("9");
    await expect(page.getByLabel(/Plan de recuperación|Recovery plan/i)).toHaveValue("Revisar la versión actual antes de continuar.");
    await expect(page.getByRole("status")).toHaveCount(0);
  });

  test("one repeated observation keeps a valid candidate visible but blocks copy/export", async ({ page }) => {
    await seedDesktop(page, { observations: 1, draft: { complete: true } });
    await page.getByRole("button", { name: /Automatizar|Automate/ }).click();
    await expect(page.getByTestId("blueprint-entry")).toBeVisible();
    await expect(page.getByTestId("blueprint-no-candidate")).toHaveCount(0);
    await page.getByTestId("blueprint-entry").click();
    await expect(page.getByTestId("blueprint-evidence-count")).toHaveText("1");
    await expect(page.getByTestId("blueprint-status")).toHaveText(/borrador|draft/i);
    await expect(page.getByTestId("blueprint-copy")).toBeDisabled();
    await expect(page.getByTestId("blueprint-export")).toBeDisabled();
  });

  test("delete confirms its boundary, restores focus on cancellation, and returns to diagnosis", async ({ page }) => {
    await seedDesktop(page, { draft: { complete: true } });
    await openBlueprint(page);
    const deleteTrigger = page.getByTestId("blueprint-delete-preview");
    const dialog = page.getByRole("dialog");

    await deleteTrigger.click();
    await expect(dialog).toContainText(/borra solo el blueprint guardado|deletes only the saved blueprint/i);
    await expect(dialog).toContainText(/no elimina exportaciones previas, contenido del portapapeles, copias movidas, backups ni copias sincronizadas|does not delete prior exports, clipboard contents, moved copies, backups, or synced copies/i);
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(deleteTrigger).toBeFocused();

    await deleteTrigger.click();
    await dialog.getByRole("button", { name: /Cancelar|Cancel/ }).click();
    await expect(dialog).toHaveCount(0);
    await expect(deleteTrigger).toBeFocused();

    await deleteTrigger.click();
    await dialog.getByRole("button", { name: /confirmar|confirm|eliminar|delete/i }).click();
    await expect(page.getByTestId("blueprint-panel")).toHaveCount(0);
    await expect(page.getByTestId("blueprint-entry")).toBeVisible();
    await expect(page.getByRole("status")).toContainText(/blueprint.*(eliminado|deleted)|(eliminado|deleted).*blueprint/i);
    const [deleted] = await invokeCalls(page, "delete_automation_blueprint_v1") ?? [];
    expect(deleted.args).toEqual({ candidateId, expectedRevision: 0, confirmation: "DELETE_AUTOMATION_BLUEPRINT", locale: "es-419" });
  });
});

test("golden vectors are available to the browser suite", async () => {
  expect(vectors.schemaVersion).toBe(1);
  expect(vectors.vectors.find((vector) => vector.name === "complete four-step workflow")?.expected).toMatchObject({ completeness: 100, weeklySavingsMinutes: 200, draftId: `blueprint-${candidateId}` });
  expect(vectors.vectors.find((vector) => vector.name === "single observation keeps a complete blueprint as draft")?.expected).toMatchObject({ completeness: 100, status: "draft" });
  expect(vectors.vectors.find((vector) => vector.name === "three recorded check-ins with two declared repetitions")?.expected).toMatchObject({ observedDays: 3, declaredRepeatedDays: 2, repeatedObservations: 2, observedWeeklyFrequency: 7, confidence: 52 });
  expect(vectors.edgeCases.oversizeDraftBytes).toBe(40001);
});
