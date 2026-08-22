import { expect, test as base, type Page } from "@playwright/test";

const STORAGE_KEY = "enso-time-web-state-v1";

type CaptureIssue =
  | "server-unavailable"
  | "window-watcher-unavailable"
  | "afk-watcher-unavailable"
  | "watcher-conflict";

const settings = {
  schemaVersion: 1,
  onboardingCompleted: true,
  locale: "es-419",
  theme: "ink",
  timezone: "America/Bogota",
  cadenceMinutes: 15,
  autostart: true,
  captureWindowTitles: true,
  retentionDays: 90,
  updateChecksEnabled: false,
  goals: ["Reducir operación manual", "Proteger tiempo de creación"],
  role: "Operador de negocio digital",
  workSchedule: { start: "08:30", end: "18:00", days: [1, 2, 3, 4, 5] },
  appRules: [],
} as const;

const pendingCheckIn = {
  id: "e2e-checkin",
  intervalStart: "2026-08-21T15:00:00.000Z",
  intervalEnd: "2026-08-21T15:15:00.000Z",
  reason: "unknown",
  status: "pending",
  createdAt: "2026-08-21T15:15:00.000Z",
} as const;

interface SeedOverrides {
  onboardingCompleted?: boolean;
  locale?: "es-419" | "en";
  captureWindowTitles?: boolean;
  captureIssue?: CaptureIssue | null;
  checkIns?: unknown[];
}

async function seedBridge(page: Page, overrides: SeedOverrides = {}) {
  const seededSettings = {
    ...settings,
    onboardingCompleted: overrides.onboardingCompleted ?? settings.onboardingCompleted,
    locale: overrides.locale ?? settings.locale,
    captureWindowTitles: overrides.captureWindowTitles ?? settings.captureWindowTitles,
  };
  await page.addInitScript(({ key, state }) => {
    if (!localStorage.getItem(key)) localStorage.setItem(key, JSON.stringify(state));
  }, {
    key: STORAGE_KEY,
    state: {
      settings: seededSettings,
      checkIns: overrides.checkIns ?? [pendingCheckIn],
      captureIssue: overrides.captureIssue,
    },
  });
}

const test = base.extend<{ browserErrors: string[] }>({
  browserErrors: async ({ page }, use) => {
    const browserErrors: string[] = [];
    page.on("console", (message) => {
      if (message.type() === "error") browserErrors.push(`console: ${message.text()}`);
    });
    page.on("pageerror", (error) => browserErrors.push(`page: ${error.message}`));
    await use(browserErrors);
    expect(browserErrors, "the browser console and page should have no errors").toEqual([]);
  },
});

test("completes onboarding and persists window-title consent", async ({ page, browserErrors: _browserErrors }) => {
  await seedBridge(page, { onboardingCompleted: false, captureWindowTitles: true, checkIns: [] });
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Entiende tu día sin llenar cronómetros." })).toBeVisible();
  await page.getByRole("button", { name: "Continuar" }).click();
  await expect(page.getByRole("heading", { name: "Completa únicamente los intervalos ambiguos." })).toBeVisible();
  await page.getByRole("button", { name: "Continuar" }).click();

  await page.getByLabel("¿Cuál es tu rol?").fill("Líder de operaciones");
  await page.getByLabel("¿Qué tiempo quieres recuperar?").fill("Reducir conciliaciones manuales");
  await page.getByRole("switch", { name: /Guardar títulos de ventana/ }).uncheck();
  await page.getByRole("button", { name: "Comenzar estudio" }).click();

  await expect(page.getByRole("heading", { name: "Así se ve tu día." })).toBeVisible();
  const saved = await page.evaluate((key) => JSON.parse(localStorage.getItem(key)!), STORAGE_KEY);
  expect(saved.settings).toMatchObject({
    onboardingCompleted: true,
    captureWindowTitles: false,
    role: "Líder de operaciones",
    goals: ["Reducir conciliaciones manuales"],
  });
});

test("surfaces every degraded capture state without exposing an engine dashboard", async ({ page, browserErrors: _browserErrors }) => {
  await seedBridge(page, { captureIssue: "server-unavailable" });
  await page.goto("/");

  const expected = new Map<CaptureIssue, RegExp>([
    ["server-unavailable", /motor local no responde/i],
    ["window-watcher-unavailable", /qué aplicación está activa/i],
    ["afk-watcher-unavailable", /cuándo te ausentas/i],
    ["watcher-conflict", /otra sesión/i],
  ]);

  for (const [captureIssue, detail] of expected) {
    await page.evaluate(({ key, captureIssue }) => {
      const state = JSON.parse(localStorage.getItem(key)!);
      localStorage.setItem(key, JSON.stringify({ ...state, captureIssue }));
    }, { key: STORAGE_KEY, captureIssue });
    await page.reload();
    await expect(page.getByRole("heading", { name: "Ensō Time no está registrando todo." })).toBeVisible();
    await expect(page.getByText(detail)).toBeVisible();
    await expect(page.locator(".content .status-pill")).toHaveText("Registro incompleto");
    await expect(page.getByRole("button", { name: "Volver a comprobar" })).toBeEnabled();
    await expect(page.locator("body")).not.toContainText("ActivityWatch");
  }

  await page.evaluate((key) => {
    const state = JSON.parse(localStorage.getItem(key)!);
    localStorage.setItem(key, JSON.stringify({ ...state, captureIssue: null }));
  }, STORAGE_KEY);
  await page.reload();
  await expect(page.getByRole("heading", { name: "Ensō Time no está registrando todo." })).toHaveCount(0);
  await expect(page.locator(".content .status-pill")).toHaveText("Registrando");
});

test("pauses and resumes tracking through the web bridge", async ({ page, browserErrors: _browserErrors }) => {
  await seedBridge(page);
  await page.goto("/");

  await page.getByRole("button", { name: "Pausar 30 min" }).click();
  await expect(page.getByRole("button", { name: "Reanudar" })).toBeVisible();
  await expect(page.locator(".content .status-pill")).toHaveText("En pausa");
  await expect.poll(() => page.evaluate((key) => Boolean(JSON.parse(localStorage.getItem(key)!).pausedUntil), STORAGE_KEY)).toBe(true);

  await page.getByRole("button", { name: "Reanudar" }).click();
  await expect(page.getByRole("button", { name: "Pausar 30 min" })).toBeVisible();
  await expect(page.locator(".content .status-pill")).toHaveText("Registrando");
});

test("answers a check-in and clears its pending count", async ({ page, browserErrors: _browserErrors }) => {
  await seedBridge(page);
  await page.goto("/");
  await page.getByRole("button", { name: "Check-ins" }).click();

  await page.getByLabel("¿Qué estabas haciendo?").fill("Preparar el reporte mensual");
  await page.getByLabel("Categoría").selectOption("admin");
  await page.getByLabel("Proyecto, opcional").fill("Operación");
  await page.getByLabel(/Es una tarea repetitiva/).check();
  await page.getByLabel(/Sigue reglas claras/).check();
  await page.getByRole("button", { name: "Guardar contexto" }).click();

  await expect(page.getByRole("heading", { name: "Todo tiene contexto." })).toBeVisible();
  await expect(page.getByText("Contexto guardado")).toBeVisible();
  await expect(page.getByText("Preparar el reporte mensual")).toBeVisible();
});

test("switches study periods, refreshes diagnostics, and copies the prompt", async ({ page, browserErrors: _browserErrors }) => {
  await seedBridge(page);
  await page.goto("/");
  await page.getByRole("button", { name: "Estudio" }).click();

  const deepReading = page.getByRole("button", { name: /28 días Lectura profunda/ });
  await deepReading.click();
  await expect(deepReading).toHaveAttribute("aria-pressed", "true");

  await page.getByRole("button", { name: "Automatizar" }).click();
  await expect(page.getByRole("heading", { name: "Qué automatizar primero." })).toBeVisible();
  await page.getByRole("button", { name: "Actualizar" }).click();
  await expect(page.getByText("Diagnóstico actualizado")).toBeVisible();
  await page.getByRole("button", { name: "Copiar prompt" }).click();
  await expect(page.getByText("Prompt copiado")).toBeVisible();
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toContain("Rol:");
});

test("saves settings, exports both formats, and deletes local data", async ({ page, browserErrors: _browserErrors }) => {
  await seedBridge(page);
  await page.goto("/");
  await page.getByRole("button", { name: "Configuración" }).click();

  await page.getByLabel("Cadencia").selectOption("30");
  await page.getByRole("button", { name: "Guardar cambios" }).click();
  await expect(page.getByText("Configuración guardada")).toBeVisible();
  await expect.poll(() => page.evaluate((key) => JSON.parse(localStorage.getItem(key)!).settings.cadenceMinutes, STORAGE_KEY)).toBe(30);

  await page.getByRole("button", { name: "Exportar JSON" }).click();
  await expect(page.getByText("Exportado en preview://enso-time-export.json")).toBeVisible();
  await page.getByRole("button", { name: "Exportar CSV" }).click();
  await expect(page.getByText("Exportado en preview://enso-time-export.csv")).toBeVisible();

  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Borrar todo" }).click();
  await expect(page.getByText("Datos eliminados")).toBeVisible();
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), STORAGE_KEY)).toBeNull();
});

test("supports keyboard navigation and basic landmarks", async ({ page, browserErrors: _browserErrors }) => {
  await seedBridge(page);
  await page.goto("/");

  await expect(page.getByRole("navigation", { name: "Principal" })).toBeVisible();
  await expect(page.getByRole("main")).toHaveAttribute("id", "main-content");
  await page.keyboard.press("Tab");
  await expect(page.locator(".skip-link")).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("main")).toBeFocused();

  await page.getByRole("button", { name: "Configuración" }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { name: "Configuración." })).toBeVisible();
  await expect(page.getByRole("switch")).toHaveCount(3);
});
