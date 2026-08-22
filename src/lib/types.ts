export type Locale = "es-419" | "en";
export type ThemeMode = "ink" | "paper" | "system";
export type CadenceMinutes = 15 | 30 | 60;
export type RetentionDays = 30 | 90 | 365 | null;
export type CategoryId =
  | "focus"
  | "communication"
  | "meetings"
  | "admin"
  | "learning"
  | "creation"
  | "personal"
  | "break"
  | "unknown";

export type CheckInReason = "unknown" | "afk" | "ambiguous";
export type CheckInStatus = "pending" | "answered" | "dismissed";
export type CaptureIssue =
  | "server-unavailable"
  | "window-watcher-unavailable"
  | "afk-watcher-unavailable"
  | "watcher-conflict";

export interface WorkSchedule {
  start: string;
  end: string;
  days: number[];
}

export interface AppRule {
  id: string;
  pattern: string;
  category: CategoryId;
  activityLabel?: string;
}

export interface SettingsV1 {
  schemaVersion: 1;
  onboardingCompleted: boolean;
  locale: Locale;
  theme: ThemeMode;
  timezone: string;
  cadenceMinutes: CadenceMinutes;
  autostart: boolean;
  captureWindowTitles: boolean;
  retentionDays: RetentionDays;
  updateChecksEnabled: boolean;
  goals: string[];
  role: string;
  workSchedule: WorkSchedule;
  appRules: AppRule[];
}

export interface TrackingStatus {
  running: boolean;
  pausedUntil?: string;
  serverPort?: number;
  captureIssue?: CaptureIssue | null;
  windowWatcher: "running" | "missing" | "stopped" | "error";
  afkWatcher: "running" | "missing" | "stopped" | "error";
  lastEventAt?: string;
  pendingCheckIns: number;
  dataStaysLocal: true;
}

export interface TimelineBlockV1 {
  id: string;
  start: string;
  end: string;
  durationSeconds: number;
  app: string;
  title?: string;
  category: CategoryId;
  confidence: number;
  afk: boolean;
  source: "window" | "checkin" | "gap";
  checkInId?: string;
}

export interface CheckInV1 {
  id: string;
  intervalStart: string;
  intervalEnd: string;
  reason: CheckInReason;
  status: CheckInStatus;
  activityLabel?: string;
  category?: CategoryId;
  project?: string;
  repetitive?: boolean;
  rulesClear?: boolean;
  createdAt: string;
  answeredAt?: string;
}

export interface CheckInAnswer {
  id: string;
  activityLabel: string;
  category: CategoryId;
  project?: string;
  repetitive: boolean;
  rulesClear: boolean;
}

export interface MetricValue {
  label: string;
  value: number;
  unit: "seconds" | "count" | "percent";
  delta?: number;
}

export interface AutomationCandidateV1 {
  id: string;
  activityLabel: string;
  estimatedWeeklyHours: number;
  weeklyFrequency: number;
  impactScore: number;
  feasibilityScore: number;
  riskScore: number;
  priorityScore: number;
  evidence: string[];
}

export interface DiagnosticV1 {
  schemaVersion: 1;
  rangeStart: string;
  rangeEnd: string;
  daysObserved: number;
  metrics: MetricValue[];
  recommendations: string[];
  candidates: AutomationCandidateV1[];
  prompt: string;
  preliminary: boolean;
  generatedAt: string;
}

export interface StudyProgress {
  startedAt: string;
  selectedDays: 7 | 14 | 28;
  observedDays: number;
  trackedHours: number;
  answeredCheckIns: number;
  coveragePercent: number;
}

export interface UpdateInfo {
  available: boolean;
  currentVersion: string;
  latestVersion?: string;
  releaseUrl?: string;
}
