import { createDiagnostic } from "./diagnostics";
import type { CaptureIssue, CheckInV1, DiagnosticV1, SettingsV1, StudyProgress, TimelineBlockV1, TrackingStatus } from "./types";

const now = new Date();
const at = (hoursAgo: number) => new Date(now.getTime() - hoursAgo * 3_600_000).toISOString();

export const demoSettings: SettingsV1 = {
  schemaVersion: 2,
  onboardingCompleted: true,
  locale: "es-419",
  theme: "ink",
  timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
  cadenceMinutes: 15,
  autostart: true,
  captureWindowTitles: true,
  retentionDays: 90,
  updateChecksEnabled: false,
  goals: ["Reducir operación manual", "Proteger tiempo de creación"],
  role: "Operador de negocio digital",
  workSchedule: { start: "08:30", end: "18:00", days: [1, 2, 3, 4, 5] },
  appRules: [],
  deepCapture: {
    browser: { enabled: false, cloudAllowed: false },
    editor: { enabled: false, cloudAllowed: false },
    shell: { enabled: false, cloudAllowed: false },
    screenshots: { enabled: false, cloudAllowed: false },
    screenshotIntervalMinutes: 15,
    browserTextAllowlist: [],
    excludedApps: ["1Password", "Bitwarden", "Passwords", "Keychain Access"],
    excludedSites: [],
  },
  ai: { enabled: false, weeklyEnabled: false, weeklyDay: 1, weeklyHour: 8 },
};

export const demoTimeline: TimelineBlockV1[] = [
  { id: "b1", start: at(7.2), end: at(6.1), durationSeconds: 3960, app: "Figma", title: "Ensō — Product system", category: "creation", confidence: 0.92, afk: false, source: "window", classificationSource: "builtin", correctable: true },
  { id: "b2", start: at(6.1), end: at(5.65), durationSeconds: 1620, app: "Slack", title: "Equipo · Operaciones", category: "communication", confidence: 0.9, afk: false, source: "window", classificationSource: "builtin", correctable: true },
  { id: "b3", start: at(5.65), end: at(4.25), durationSeconds: 5040, app: "Visual Studio Code", title: "automation-workflow.ts", category: "focus", confidence: 0.98, afk: false, source: "window", classificationSource: "builtin", correctable: true },
  { id: "b4", start: at(4.25), end: at(3.65), durationSeconds: 2160, app: "Away", category: "meetings", confidence: 1, afk: true, source: "checkin", checkInId: "c1", classificationSource: "afk", correctable: false },
  { id: "b5", start: at(3.65), end: at(2.8), durationSeconds: 3060, app: "Google Chrome", title: "Make — Scenario history", category: "admin", confidence: 0.72, afk: false, source: "window", classificationSource: "builtin", correctable: true },
  { id: "b6", start: at(2.8), end: at(1.55), durationSeconds: 4500, app: "Notion", title: "SOP · Publicación semanal", category: "focus", confidence: 0.87, afk: false, source: "window", classificationSource: "builtin", correctable: true },
  { id: "b7", start: at(1.55), end: at(0.8), durationSeconds: 2700, app: "Google Chrome", title: "Untitled", category: "unknown", confidence: 0.2, afk: false, source: "window", checkInId: "c2", classificationSource: "unknown", correctable: true },
  { id: "b8", start: at(0.8), end: at(0.1), durationSeconds: 2520, app: "Arc", title: "ChatGPT", category: "learning", confidence: 0.64, afk: false, source: "window", classificationSource: "builtin", correctable: true },
];

export const demoCheckIns: CheckInV1[] = [
  { id: "c1", intervalStart: at(4.25), intervalEnd: at(3.65), reason: "afk", status: "answered", activityLabel: "Reunión semanal con operaciones", category: "meetings", project: "Operación", repetitive: true, rulesClear: true, createdAt: at(3.65), answeredAt: at(3.6) },
  { id: "c2", intervalStart: at(1.55), intervalEnd: at(0.8), reason: "unknown", status: "pending", createdAt: at(0.8) },
  { id: "c3", intervalStart: at(30), intervalEnd: at(29.75), reason: "ambiguous", status: "answered", activityLabel: "Actualizar reporte semanal", category: "admin", repetitive: true, rulesClear: true, createdAt: at(29.75), answeredAt: at(29.7) },
  { id: "c4", intervalStart: at(54), intervalEnd: at(53.75), reason: "ambiguous", status: "answered", activityLabel: "Actualizar reporte semanal", category: "admin", repetitive: true, rulesClear: true, createdAt: at(53.75), answeredAt: at(53.7) },
  { id: "c5", intervalStart: at(78), intervalEnd: at(77.75), reason: "ambiguous", status: "answered", activityLabel: "Actualizar reporte semanal", category: "admin", repetitive: true, rulesClear: true, createdAt: at(77.75), answeredAt: at(77.7) },
];

export const demoStatus: TrackingStatus = {
  running: true,
  serverPort: 5618,
  windowWatcher: "running",
  afkWatcher: "running",
  lastEventAt: at(0.02),
  pendingCheckIns: 1,
  dataStaysLocal: true,
  processingMode: "local-only",
  experimentalIntelligence: false,
};

export function demoTrackingStatus(captureIssue?: CaptureIssue | null): TrackingStatus {
  return { ...demoStatus, captureIssue };
}

export const demoStudy: StudyProgress = {
  startedAt: at(24 * 5.4),
  selectedDays: 7,
  observedDays: 5.4,
  trackedHours: 31.8,
  answeredCheckIns: 14,
  coveragePercent: 78,
};

export const demoDiagnostic: DiagnosticV1 = createDiagnostic(demoTimeline, demoCheckIns, 5, demoSettings.role, demoSettings.goals);

/** Preview-only stand-in for the desktop episode analysis (the web demo has no capture). */
export const demoObserved = {
  episodes: 4,
  observedDays: 3,
  medianMinutes: 14,
  variants: 2,
  topVariantShare: 0.75,
  topVariantSteps: ["mail.google.com", "excel", "app.hubspot.com"],
  cases: 3,
};

export const demoCaptureHealth = {
  activeSeconds: 6 * 3600,
  unknownShare: 0.31,
  topUnknown: [{ app: "microsoft edge", seconds: 2 * 3600 }, { app: "preview", seconds: 600 }],
  titles: "missing" as const,
  titledShare: 0,
  accessibilityFix: true,
  browserSeconds: 2 * 3600,
  browserDomainShare: 0,
  browserConnector: "not-paired" as const,
  coverage: {
    trackingSince: "2026-11-20T13:00:00Z",
    schedule: { captured: 30 * 3600, idle: 3600, asleep: 2 * 3600, quit: 1800, closedOrOff: 6 * 3600, watcherDown: 3600, paused: 0 },
    all: { captured: 34 * 3600, idle: 2 * 3600, asleep: 40 * 3600, quit: 1800, closedOrOff: 20 * 3600, watcherDown: 3600, paused: 0 },
    gaps: [
      { start: "2026-11-18T13:30:00Z", end: "2026-11-18T19:30:00Z", reason: "closed-or-off" as const, inferred: true, scheduleSeconds: 6 * 3600 },
      { start: "2026-11-21T15:00:00Z", end: "2026-11-21T16:00:00Z", reason: "watcher-down" as const, inferred: false, scheduleSeconds: 3600 },
    ],
  },
};

export const demoCaptureGapNotice = {
  start: "2026-11-21T14:00:00Z",
  end: "2026-11-21T16:30:00Z",
  reason: "closed-or-off" as const,
  inferred: false,
  scheduleSeconds: 9000,
};

export const demoHandoffs = [
  { from: "mail.google.com", to: "excel", count: 4 },
  { from: "excel", to: "app.hubspot.com", count: 3 },
];
