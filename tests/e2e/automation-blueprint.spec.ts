import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";

const vectors = JSON.parse(readFileSync(new URL("../fixtures/automation-blueprint-vectors.json", import.meta.url), "utf8")) as {
  schemaVersion: number;
  vectors: Array<{ name: string; expected: Record<string, number | string> }>;
  edgeCases: { oversizeDraftBytes: number };
};

/**
 * v0.2 UI contract. These test ids are intentionally stable and are the
 * hand-off between the blueprint frontend and this E2E suite:
 * blueprint-entry, blueprint-panel, blueprint-status, blueprint-completeness,
 * blueprint-source-map, blueprint-boundary, blueprint-copy, blueprint-export,
 * blueprint-refresh, blueprint-delete-preview, blueprint-error,
 * blueprint-conflict, blueprint-loading, blueprint-evidence-count.
 */
const STORAGE_KEY = "enso-time-web-state-v1";
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

async function seed(page: Page, options: { locale?: "es-419" | "en"; blueprint?: unknown; intakes?: unknown[]; observations?: number } = {}) {
  await page.addInitScript(({ key, locale, blueprint, intakes, observations }) => {
    localStorage.setItem(key, JSON.stringify({
      settings: { onboardingCompleted: true, locale, theme: "ink", timezone: "America/Bogota", cadenceMinutes: 15, autostart: true, captureWindowTitles: true, retentionDays: 90, updateChecksEnabled: false, goals: ["Recuperar tiempo"], role: "Operaciones", workSchedule: { start: "08:30", end: "18:00", days: [1, 2, 3, 4, 5] }, appRules: [] },
      checkIns: [],
      workflowIntakes: intakes,
      automationBlueprints: blueprint ? [blueprint] : [],
      automationObservations: observations,
      captureIssue: null,
    }));
  }, { key: STORAGE_KEY, locale: options.locale ?? "es-419", blueprint: options.blueprint, intakes: options.intakes ?? [intake], observations: options.observations ?? 2 });
  await page.goto("/");
}

async function openBlueprint(page: Page) {
  await page.getByRole("button", { name: /Automatizar|Automate/ }).click();
  await page.getByTestId("blueprint-entry").click();
  await expect(page.getByTestId("blueprint-panel")).toBeVisible();
}

test.describe("Automation Blueprint v0.2 contract", () => {
  test("ES complete journey: intake → draft → missing → ready → copy/export", async ({ page }) => {
    await seed(page);
    await openBlueprint(page);

    await expect(page.getByTestId("blueprint-status")).toHaveText(/borrador|incompleto/i);
    await expect(page.getByTestId("blueprint-source-map")).toContainText(Spanish.trigger);
    await expect(page.getByTestId("blueprint-source-map")).toContainText(Spanish.output);
    await expect(page.getByTestId("blueprint-evidence-count")).toHaveText(/2/);
    await expect(page.getByTestId("blueprint-copy")).toBeDisabled();
    await expect(page.getByTestId("blueprint-export")).toBeDisabled();

    await page.getByLabel(/Frecuencia semanal|Runs per week/i).fill("5");
    await page.getByLabel(/Minutos de verificación|Verification minutes/i).fill("5");
    await page.getByLabel(/Sensibilidad de datos|Data sensitivity/i).selectOption("low");
    await page.getByLabel(/Efectos secundarios|Side effects/i).selectOption("reversible");
    await page.getByLabel(/Modo de verificación|Verification mode/i).selectOption("every-run");
    await page.getByLabel(/Plan de recuperación|Recovery plan/i).fill("Revisar el CSV y repetir la validación manual.");
    const boundary = page.getByTestId("blueprint-boundary").first();
    await boundary.getByLabel(/Clasificación|Disposition/i).selectOption("automatable");
    await boundary.getByLabel(/Racional|Rationale/i).fill("Reglas repetibles y reversibles.");
    await page.getByTestId("blueprint-save").click();

    await expect(page.getByTestId("blueprint-status")).toHaveText(/listo|ready/i);
    await expect(page.getByTestId("blueprint-completeness")).toHaveText(/100/);
    await expect(page.getByTestId("blueprint-assumptions")).toContainText(/igual.*tiempo|equal-time/i);
    await expect(page.getByTestId("blueprint-assumptions")).toContainText(/estimaci[oó]n|planning estimate/i);
    await expect(page.getByTestId("blueprint-copy")).toBeEnabled();
    await expect(page.getByTestId("blueprint-export")).toBeEnabled();
    await page.getByTestId("blueprint-copy").click();
    await expect(page.getByRole("status")).toContainText(/copiado|copied/i);
    await page.getByTestId("blueprint-export").click();
    await expect(page.getByRole("status")).toContainText(/exportado|exported/i);
    await expect(page.locator("body")).not.toContainText(/OAuth|token|credencial|ejecutar flujo|run workflow/i);
  });

  test("English changes chrome only and preserves Spanish intake content", async ({ page }) => {
    await seed(page, { locale: "en" });
    await openBlueprint(page);
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
    await expect(page.getByTestId("blueprint-source-map")).toContainText(Spanish.trigger);
    await expect(page.getByTestId("blueprint-source-map")).toContainText(Spanish.human);
    await expect(page.getByTestId("blueprint-source-map")).not.toContainText("Llega el cierre semanal de ventas traducido");
    await expect(page.locator("a")).toHaveCount(0);
    await expect(page.locator("input")).not.toHaveAttribute("name", /oauth|token|secret|credential/i);
  });

  test("mobile and keyboard users can complete boundaries with visible focus", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await seed(page);
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

  test("loading, backend error, stale source and optimistic conflict are explicit", async ({ page }) => {
    await seed(page, { blueprint: { candidateId, status: "stale", revision: 3, sourceIntakeUpdatedAt: "2026-08-22T12:00:00.000Z" } });
    await openBlueprint(page);
    await expect(page.getByTestId("blueprint-status")).toHaveText(/desactualizado|stale/i);
    await expect(page.getByTestId("blueprint-refresh")).toBeEnabled();
    await expect(page.getByTestId("blueprint-copy")).toBeDisabled();
    await expect(page.getByTestId("blueprint-export")).toBeDisabled();
    await page.getByTestId("blueprint-refresh").click();
    await expect(page.getByTestId("blueprint-refresh-confirmation")).toBeVisible();

    await page.evaluate((key) => {
      const state = JSON.parse(localStorage.getItem(key)!);
      localStorage.setItem(key, JSON.stringify({ ...state, automationBlueprintError: "backend unavailable" }));
    }, STORAGE_KEY);
    await page.reload();
    await expect(page.getByTestId("blueprint-error")).toContainText(/backend|disponible|unavailable/i);
    await expect(page.getByTestId("blueprint-retry")).toBeEnabled();

    await page.evaluate((key) => {
      const state = JSON.parse(localStorage.getItem(key)!);
      localStorage.setItem(key, JSON.stringify({ ...state, automationBlueprintError: null, automationBlueprintConflict: { currentRevision: 4 } }));
    }, STORAGE_KEY);
    await page.reload();
    await expect(page.getByTestId("blueprint-conflict")).toContainText(/conflicto|conflict/i);
    await expect(page.getByTestId("blueprint-conflict")).toContainText("4");
    await expect(page.getByTestId("blueprint-save")).toBeDisabled();
  });

  test("insufficient evidence has no blueprint entry, and delete is preview-only", async ({ page }) => {
    await seed(page, { intakes: [], observations: 0 });
    await page.getByRole("button", { name: /Automatizar|Automate/ }).click();
    await expect(page.getByTestId("blueprint-no-candidate")).toBeVisible();
    await expect(page.getByTestId("blueprint-entry")).toHaveCount(0);

    await seed(page, { blueprint: { candidateId, status: "ready", revision: 2 } });
    await openBlueprint(page);
    await page.getByTestId("blueprint-delete-preview").click();
    await expect(page.getByRole("dialog")).toContainText(/clipboard|portapapeles|copias|backups/i);
    await expect(page.getByRole("dialog")).toContainText(/no.*ejecut|not.*run/i);
    await page.getByRole("dialog").getByRole("button", { name: /Cancelar|Cancel/ }).click();
    await expect(page.getByTestId("blueprint-panel")).toBeVisible();
  });
});

test("golden vectors are available to the browser suite", async () => {
  expect(vectors.schemaVersion).toBe(1);
  expect(vectors.vectors.find((vector) => vector.name === "complete four-step workflow")?.expected).toMatchObject({ completeness: 100, weeklySavingsMinutes: 200 });
  expect(vectors.edgeCases.oversizeDraftBytes).toBe(40001);
});
