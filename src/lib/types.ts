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
  | "watcher-conflict"
  | "accessibility-permission-required";

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

export interface SourcePolicyV2 {
  enabled: boolean;
  cloudAllowed: boolean;
}

export interface DeepCaptureSettingsV2 {
  browser: SourcePolicyV2;
  editor: SourcePolicyV2;
  shell: SourcePolicyV2;
  screenshots: SourcePolicyV2;
  screenshotIntervalMinutes: 5 | 15 | 30 | 60;
  browserTextAllowlist: string[];
  excludedApps: string[];
  excludedSites: string[];
}

export interface AiSettingsV2 {
  enabled: boolean;
  analystModel?: string;
  evaluatorModel?: string;
  monthlyBudgetUsd?: number;
  weeklyEnabled: boolean;
  weeklyDay: number;
  weeklyHour: number;
}

/** `domain` rules store a bare host in `app` and match browser sessions with connector context. */
export type ClassificationRuleScope = "app" | "app-title" | "domain" | "legacy";
export type ClassificationSource = "afk" | "override" | "checkin" | "rule" | "builtin" | "unknown";

export interface ClassificationRuleV1 {
  id: string;
  scope: ClassificationRuleScope;
  app: string;
  titlePattern?: string;
  category: CategoryId;
  activityLabel?: string;
  createdAt: string;
  updatedAt: string;
}

export interface RuleImpactV1 {
  sessions: number;
  durationSeconds: number;
}

export interface RuleSuggestionV1 {
  rule: ClassificationRuleV1;
  impact: RuleImpactV1;
}

export interface SessionClassificationInputV1 {
  blockId: string;
  category: CategoryId;
  activityLabel?: string;
}

export interface SessionClassificationResultV1 {
  block: TimelineBlockV1;
  suggestion?: RuleSuggestionV1;
}

export interface ReviewGroupV1 {
  id: string;
  app: string;
  titlePattern?: string;
  sampleTitle?: string;
  sessions: number;
  durationSeconds: number;
  firstSeen: string;
  lastSeen: string;
  browserFallback: boolean;
  /** Set when browser sessions are grouped by connector domain instead of title. */
  domain?: string;
}

export interface ApplyReviewGroupInputV1 {
  from: string;
  to: string;
  app: string;
  titlePattern?: string;
  category: CategoryId;
  activityLabel?: string;
  mode: "sessions" | "rule";
  domain?: string;
}

export interface ReviewOperationV1 {
  id: string;
  mode: "sessions" | "rule";
  affectedSessions: number;
  blockIds: string[];
  ruleId?: string;
}

/** An episode with no confirmed label, offered for one-click labeling at day's end. */
export interface EpisodeSuggestionV1 {
  id: string;
  start: string;
  end: string;
  steps: string[];
  touchSeconds: number;
  blockIds: string[];
}

export interface LabelEpisodeInputV1 {
  from: string;
  to: string;
  blockIds: string[];
  category: CategoryId;
  activityLabel: string;
}

export interface UndoReviewOperationInputV1 {
  blockIds: string[];
  ruleId?: string;
}

export interface SettingsV1 {
  schemaVersion: 2;
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
  deepCapture: DeepCaptureSettingsV2;
  ai: AiSettingsV2;
}

export interface TrackingStatus {
  running: boolean;
  pausedUntil?: string;
  serverPort?: number;
  captureIssue?: CaptureIssue | null;
  nativeActionError?: string | null;
  windowWatcher: "running" | "missing" | "stopped" | "error";
  afkWatcher: "running" | "missing" | "stopped" | "error";
  lastEventAt?: string;
  pendingCheckIns: number;
  dataStaysLocal: boolean;
  processingMode: "local-only" | "cloud-opt-in";
  lastCloudRunAt?: string;
  /** OpenRouter evaluation and screen OCR are built in only with the v0.3 preview flag. */
  experimentalIntelligence: boolean;
}

export interface TimelineBlockV1 {
  id: string;
  start: string;
  end: string;
  durationSeconds: number;
  app: string;
  title?: string;
  activityLabel?: string;
  category: CategoryId;
  confidence: number;
  afk: boolean;
  source: "window" | "checkin" | "gap";
  checkInId?: string;
  classificationSource?: ClassificationSource;
  ruleId?: string;
  correctable?: boolean;
  /** Browser domain from a paired connector that covered most of the block. */
  contextDomain?: string;
  /** Editor or shell project from a paired connector that covered most of the block. */
  contextProject?: string;
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
  /** Observed by the capture (work episodes), kept apart from declared evidence. */
  observed?: ObservedEvidenceV1;
}

export interface ObservedEvidenceV1 {
  episodes: number;
  observedDays: number;
  medianMinutes: number;
  /** Present only once the task has at least three observed episodes. */
  variants?: number;
  topVariantShare?: number;
  topVariantSteps?: string[];
  /** Distinct hashed tickets/invoices/orders seen in the task's episodes. */
  cases?: number;
}

export interface CaptureHealthV1 {
  activeSeconds: number;
  unknownShare: number;
  topUnknown: { app: string; seconds: number }[];
  titles: "off" | "missing" | "ok" | "insufficient";
  titledShare: number;
  /** Missing titles are fixed by granting Accessibility (macOS only). */
  accessibilityFix: boolean;
  browserSeconds: number;
  browserDomainShare: number;
  browserConnector: "not-paired" | "not-reporting" | "ok" | "insufficient";
}

export interface HandoffV1 {
  from: string;
  to: string;
  count: number;
}

export interface WorkflowIntakeV1 {
  id: string;
  candidateId: string;
  candidateLabel: string;
  trigger: string;
  tools: string[];
  steps: string[];
  humanDecisions: string;
  inputs: string;
  desiredOutput: string;
  frequency: string;
  minutesPerRun: number;
  painPoints: string;
  createdAt: string;
  updatedAt: string;
}

export type StepDispositionV1 = "unassessed" | "automatable" | "human-required";
export type DataSensitivityV1 = "unassessed" | "low" | "moderate" | "high";
export type SideEffectLevelV1 = "unassessed" | "none" | "reversible" | "irreversible";
export type VerificationModeV1 = "unassessed" | "none" | "sample" | "every-run";
export type AutomationBlueprintStatusV1 = "draft" | "ready" | "stale" | "blocked";

export interface StepBoundaryV1 {
  stepId: string;
  disposition: StepDispositionV1;
  rationale?: string;
}

export interface AutomationBlueprintDraftV1 {
  schemaVersion: 1;
  id: string;
  candidateId: string;
  workflowIntakeId: string;
  sourceIntakeUpdatedAt: string;
  revision: number;
  runsPerWeek?: number;
  stepBoundaries: StepBoundaryV1[];
  dataSensitivity: DataSensitivityV1;
  sideEffectLevel: SideEffectLevelV1;
  verificationMode: VerificationModeV1;
  verificationMinutesPerRun?: number;
  recoveryPlan?: string;
  createdAt: string;
  updatedAt: string;
}

export interface BlueprintSourceMapV1 {
  trigger: string;
  inputs: string;
  desiredOutput: string;
  tools: string[];
  steps: Array<{ id: string; label: string }>;
  humanDecisions: string;
  minutesPerRun: number;
  frequency: string;
  painPoints: string;
}

export interface BlueprintEvidenceV1 {
  observed: string[];
  declared: string[];
  repeatedObservations: number;
  observedDays: number;
  declaredRepeatedDays: number;
  rulesQuality: number;
  observedWeeklyFrequency: number;
  /** Work episodes from the latest diagnostic. Informational: never changes confidence or status. */
  observedEpisodes?: ObservedEvidenceV1 & { computedAt: string };
}

export interface BlueprintCompletenessV1 {
  score: number;
  missing: string[];
}

export interface BlueprintEstimatesV1 {
  weeklyBaselineMinutes: number;
  weeklyResidualMinutes: number;
  weeklySavingsMinutes: number;
  effort: number;
  operationalRisk: number;
  confidence: number;
  components: Record<string, number>;
}

export interface AutomationBlueprintV1 {
  schemaVersion: 1;
  draft: AutomationBlueprintDraftV1;
  source: BlueprintSourceMapV1;
  evidence: BlueprintEvidenceV1;
  status: AutomationBlueprintStatusV1;
  completeness: BlueprintCompletenessV1;
  estimates: BlueprintEstimatesV1;
  renderedMarkdown: string;
  conflict?: { currentRevision: number };
}

export interface SaveAutomationBlueprintInputV1 {
  draft: AutomationBlueprintDraftV1;
  expectedRevision?: number;
}

export interface DiagnosticV1 {
  schemaVersion: 1;
  rangeStart: string;
  rangeEnd: string;
  daysObserved: number;
  metrics: MetricValue[];
  recommendations: string[];
  candidates: AutomationCandidateV1[];
  /** Most repeated app/site handoffs inside work episodes. Absent in older payloads. */
  handoffs?: HandoffV1[];
  /** How complete the captured data is. Native only; absent in older payloads. */
  captureHealth?: CaptureHealthV1;
  prompt: string;
  preliminary: boolean;
  generatedAt: string;
}

export interface EvidenceItemV1 {
  id: string;
  kind: string;
  label: string;
  category?: string;
  durationSeconds: number;
  occurrences: number;
  apps: string[];
  context: string[];
}

export interface EvidenceBundleV1 {
  schemaVersion: 1;
  generatedAt: string;
  rangeStart: string;
  rangeEnd: string;
  daysObserved: number;
  role: string;
  goals: string[];
  sources: string[];
  evidence: EvidenceItemV1[];
}

export interface EvaluationPayloadPreviewV1 {
  policyFingerprint: string;
  sources: string[];
  estimatedInputTokens: number;
  estimatedMaxCostUsd: number;
  budgetRemainingUsd: number;
  consentCurrent: boolean;
  payload: EvidenceBundleV1;
}

export interface OpenRouterModelV1 {
  id: string;
  name: string;
  contextLength: number;
  promptPrice: number;
  completionPrice: number;
  supportsStructuredOutput: boolean;
}

export type AiSolutionLevel = "prompt" | "assistant" | "automation" | "agent" | "do-not-automate";

export interface AnalysisOpportunityV1 {
  id: string;
  title: string;
  candidateId?: string;
  problem: string;
  solutionLevel: AiSolutionLevel;
  evidenceIds: string[];
  rationale: string;
  experiment: string;
  successMetric: string;
}

export interface EvaluationVerdictItemV1 {
  opportunityId: string;
  faithfulness: number;
  specificity: number;
  feasibility: number;
  impact: number;
  safety: number;
  approved: boolean;
  reasons: string[];
}

export interface PublishedOpportunityV1 extends AnalysisOpportunityV1 {
  estimatedWeeklyHours: number;
  evaluator: EvaluationVerdictItemV1;
}

export interface EvaluationReportV1 {
  schemaVersion: 1;
  id: string;
  trigger: "manual" | "weekly";
  generatedAt: string;
  analystModel: string;
  evaluatorModel: string;
  summary: string;
  opportunities: PublishedOpportunityV1[];
  rejectedCount: number;
  inputTokens: number;
  outputTokens: number;
  costUsd: number;
}

export interface AiConfigurationStatusV1 {
  hasApiKey: boolean;
  ready: boolean;
  consentCurrent: boolean;
  spentThisMonthUsd: number;
  budgetUsd?: number;
  analystModel?: string;
  evaluatorModel?: string;
}

export interface ConnectorPairingV1 {
  code: string;
  kind: "browser" | "editor" | "shell";
  name: string;
  origin?: string;
  expiresAt: string;
  serverPort?: number;
}

export interface ConnectorClientV1 {
  id: string;
  kind: "browser" | "editor" | "shell";
  name: string;
  origin?: string;
  createdAt: string;
  lastSeenAt?: string;
  revokedAt?: string;
}

export interface ScreenOcrStatusV1 {
  supported: boolean;
  permission: "granted" | "required" | "unsupported" | string;
  lastObservationAt?: string;
  lastError?: string;
}

export interface AiFeedbackInputV1 {
  runId: string;
  opportunityId: string;
  rating: "useful" | "not-useful" | "edited" | "needs-context";
  note?: string;
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
