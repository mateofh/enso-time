import { invoke } from "@tauri-apps/api/core";
import { demoCheckIns, demoSettings, demoStudy, demoTimeline, demoTrackingStatus } from "./demo";
import { buildPrompt, createDiagnostic } from "./diagnostics";
import { migrateWorkflowIntakes, upsertWorkflowIntake } from "./workflowIntakes";
import { buildPreviewBlueprint, emptyBlueprintDraft } from "./blueprints";
import { canonicalCandidateLabel } from "./candidateIdentity";
import type {
  AutomationBlueprintDraftV1,
  AutomationBlueprintV1,
  CheckInAnswer,
  CheckInV1,
  CaptureIssue,
  DiagnosticV1,
  SettingsV1,
  StudyProgress,
  TimelineBlockV1,
  TrackingStatus,
  UpdateInfo,
  WorkflowIntakeV1,
  SaveAutomationBlueprintInputV1,
  BlueprintEvidenceV1,
} from "./types";

const STORAGE_KEY = "enso-time-web-state-v1";

interface WebState {
  settings: SettingsV1;
  checkIns: CheckInV1[];
  pausedUntil?: string;
  captureIssue?: CaptureIssue | null;
  workflowIntakes?: WorkflowIntakeV1[];
  automationBlueprints?: AutomationBlueprintDraftV1[];
  automationBlueprintError?: string | null;
  automationBlueprintConflict?: { currentRevision: number } | null;
}

export class BlueprintBridgeError extends Error {
  constructor(public readonly code: string, public readonly current?: AutomationBlueprintV1) {
    super(code);
  }
}

function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

function loadWebState(): WebState {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
      const state = JSON.parse(stored) as WebState;
      if (!Array.isArray(state.workflowIntakes)) return state;
      const workflowIntakes = migrateWorkflowIntakes(state.workflowIntakes);
      const migrated = { ...state, workflowIntakes };
      if (JSON.stringify(migrated) !== stored) saveWebState(migrated);
      return migrated;
    }
  } catch {
    // A corrupt preview state should never prevent the UI from loading.
  }
  return { settings: demoSettings, checkIns: demoCheckIns };
}

function saveWebState(state: WebState) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
}

function utcDay(timestamp: string): string {
  const milliseconds = Date.parse(timestamp);
  return Number.isFinite(milliseconds) ? new Date(milliseconds).toISOString().slice(0, 10) : timestamp.slice(0, 10);
}

export function previewBlueprintEvidence(intake: WorkflowIntakeV1, checkIns: CheckInV1[], locale: string): BlueprintEvidenceV1 {
  const candidateLabel = canonicalCandidateLabel(intake.candidateLabel);
  const matchingCheckIns = checkIns.filter((item) =>
    item.status === "answered" && Boolean(item.activityLabel) && canonicalCandidateLabel(item.activityLabel!) === candidateLabel,
  );
  const repeatedCheckIns = matchingCheckIns.filter((item) => item.repetitive === true);
  const observedDays = new Set(matchingCheckIns.map((item) => utcDay(item.intervalStart))).size;
  const declaredRepeatedDays = new Set(repeatedCheckIns.map((item) => utcDay(item.intervalStart))).size;
  const rulesQuality = repeatedCheckIns.length
    ? repeatedCheckIns.filter((item) => item.rulesClear === true).length / repeatedCheckIns.length
    : 0;
  const observed = matchingCheckIns.map((item) => locale === "en"
    ? `Recorded check-in interval: ${item.intervalStart}–${item.intervalEnd}`
    : `Intervalo de check-in registrado: ${item.intervalStart}–${item.intervalEnd}`);
  const declared = [locale === "en" ? "Workflow intake supplied by you" : "Intake del workflow declarado por ti"];
  for (const item of matchingCheckIns) {
    declared.push(locale === "en" ? `Activity label confirmed by you: ${item.activityLabel}` : `Etiqueta de actividad confirmada por ti: ${item.activityLabel}`);
    declared.push(item.repetitive === true
      ? (locale === "en" ? "Repetition confirmed by you" : "Repetición confirmada por ti")
      : (locale === "en" ? "You did not confirm this check-in as repetitive" : "No confirmaste este check-in como repetitivo"));
    declared.push(item.rulesClear === true
      ? (locale === "en" ? "Clear rules confirmed by you" : "Reglas claras confirmadas por ti")
      : (locale === "en" ? "Clear rules not confirmed by you" : "Reglas claras no confirmadas por ti"));
  }
  if (repeatedCheckIns.length) {
    declared.push(locale === "en"
      ? `Repetition frequency confirmed by you: ${repeatedCheckIns.length} confirmation(s) across ${declaredRepeatedDays} declared UTC day(s)`
      : `Frecuencia de repetición confirmada por ti: ${repeatedCheckIns.length} confirmación(es) en ${declaredRepeatedDays} día(s) UTC declarados`);
  }
  return {
    observed,
    declared,
    repeatedObservations: repeatedCheckIns.length,
    observedDays,
    declaredRepeatedDays,
    rulesQuality,
    // This wire name is retained for compatibility; it is derived exclusively
    // from repetitions and days explicitly confirmed by the person.
    observedWeeklyFrequency: declaredRepeatedDays ? repeatedCheckIns.length * 7 / declaredRepeatedDays : 0,
  };
}

function enrichPreviewDiagnostic(diagnostic: DiagnosticV1, state: WebState): DiagnosticV1 {
  const candidateId = diagnostic.candidates[0]?.id;
  const intake = (state.workflowIntakes ?? []).find((item) => item.candidateId === candidateId);
  return { ...diagnostic, prompt: buildPrompt(diagnostic.candidates[0], state.settings.role, state.settings.goals, diagnostic.daysObserved, state.settings.locale, intake) };
}

export const bridge = {
  async completeInstallerSmokeTest(): Promise<void> {
    if (isTauri()) await invoke("complete_installer_smoke_test");
  },
  shouldResumeAutomationBlueprint(): boolean {
    if (isTauri()) return false;
    const state = loadWebState();
    return Boolean(state.automationBlueprintError || state.automationBlueprintConflict);
  },

  async getSettings(): Promise<SettingsV1> {
    return isTauri() ? invoke("get_settings") : loadWebState().settings;
  },

  async saveSettings(settings: SettingsV1): Promise<SettingsV1> {
    if (isTauri()) return invoke("save_settings", { settings });
    const state = loadWebState();
    saveWebState({ ...state, settings });
    return settings;
  },

  async getTrackingStatus(): Promise<TrackingStatus> {
    if (isTauri()) return invoke("get_tracking_status");
    const state = loadWebState();
    return {
      ...demoTrackingStatus(state.captureIssue),
      running: !state.pausedUntil || new Date(state.pausedUntil) <= new Date(),
      pausedUntil: state.pausedUntil,
      pendingCheckIns: state.checkIns.filter((item) => item.status === "pending").length,
    };
  },

  async getTimeline(from: string, to: string): Promise<TimelineBlockV1[]> {
    return isTauri() ? invoke("get_timeline", { from, to }) : demoTimeline;
  },

  async listCheckIns(): Promise<CheckInV1[]> {
    return isTauri() ? invoke("list_checkins") : loadWebState().checkIns;
  },

  async answerCheckIn(answer: CheckInAnswer): Promise<CheckInV1> {
    if (isTauri()) return invoke("answer_checkin", { answer });
    const state = loadWebState();
    const updated = state.checkIns.map((item) => item.id === answer.id ? { ...item, ...answer, status: "answered" as const, answeredAt: new Date().toISOString() } : item);
    saveWebState({ ...state, checkIns: updated });
    return updated.find((item) => item.id === answer.id)!;
  },

  async getStudyProgress(): Promise<StudyProgress> {
    return isTauri() ? invoke("get_study_progress") : demoStudy;
  },

  async generateDiagnostic(days: 7 | 14 | 28): Promise<DiagnosticV1> {
    if (isTauri()) return invoke("generate_diagnostic", { days });
    const state = loadWebState();
    const diagnostic = createDiagnostic(demoTimeline, state.checkIns, Math.min(days, 5), state.settings.role, state.settings.goals, state.settings.locale);
    return enrichPreviewDiagnostic(diagnostic, state);
  },

  async listWorkflowIntakes(): Promise<WorkflowIntakeV1[]> {
    return isTauri() ? invoke("list_workflow_intakes") : loadWebState().workflowIntakes ?? [];
  },

  async saveWorkflowIntake(intake: WorkflowIntakeV1): Promise<WorkflowIntakeV1> {
    if (isTauri()) return invoke("save_workflow_intake", { intake });
    const state = loadWebState();
    const result = upsertWorkflowIntake(state.workflowIntakes ?? [], intake);
    saveWebState({ ...state, workflowIntakes: result.items });
    return result.persisted;
  },

  async getAutomationBlueprint(candidateId: string, locale: string): Promise<AutomationBlueprintV1 | null> {
    if (isTauri()) return invoke("get_automation_blueprint_v1", { candidateId, locale });
    const state = loadWebState();
    if (state.automationBlueprintError) throw new Error(state.automationBlueprintError);
    const intake = (state.workflowIntakes ?? []).find((item) => item.candidateId === candidateId);
    if (!intake) return null;
    const raw = state.automationBlueprints?.find((item) => item.candidateId === candidateId);
    const base = emptyBlueprintDraft(intake);
    const draft: AutomationBlueprintDraftV1 = raw ? { ...base, ...raw, candidateId, workflowIntakeId: raw.workflowIntakeId ?? intake.id, sourceIntakeUpdatedAt: raw.sourceIntakeUpdatedAt ?? intake.updatedAt, revision: raw.revision ?? 0, stepBoundaries: raw.stepBoundaries ?? base.stepBoundaries } : base;
    const evidence = previewBlueprintEvidence(intake, state.checkIns, locale);
    return { ...buildPreviewBlueprint(intake, draft, evidence), conflict: state.automationBlueprintConflict ?? undefined };
  },

  async saveAutomationBlueprint(input: SaveAutomationBlueprintInputV1, locale: string): Promise<AutomationBlueprintV1> {
    if (isTauri()) return invoke("save_automation_blueprint_v1", { input, locale });
    const state = loadWebState();
    const intake = (state.workflowIntakes ?? []).find((item) => item.candidateId === input.draft.candidateId);
    if (!intake) throw new BlueprintBridgeError("blueprint.intake_missing");
    const existing = state.automationBlueprints?.find((item) => item.candidateId === input.draft.candidateId);
    if (input.expectedRevision !== undefined && input.expectedRevision !== (existing?.revision ?? 0)) {
      throw new BlueprintBridgeError("blueprint.conflict", existing ? buildPreviewBlueprint(intake, existing, previewBlueprintEvidence(intake, state.checkIns, locale)) : undefined);
    }
    const now = new Date().toISOString();
    const draft: AutomationBlueprintDraftV1 = { ...input.draft, revision: (existing?.revision ?? 0) + 1, updatedAt: now, createdAt: existing?.createdAt ?? input.draft.createdAt };
    const automationBlueprints = [...(state.automationBlueprints ?? []).filter((item) => item.candidateId !== draft.candidateId), draft];
    saveWebState({ ...state, automationBlueprints });
    return buildPreviewBlueprint(intake, draft, previewBlueprintEvidence(intake, state.checkIns, locale));
  },

  async refreshAutomationBlueprintSource(candidateId: string, expectedRevision: number, locale: string): Promise<AutomationBlueprintV1> {
    if (isTauri()) return invoke("refresh_automation_blueprint_source_v1", { candidateId, expectedRevision, locale });
    const state = loadWebState();
    const intake = (state.workflowIntakes ?? []).find((item) => item.candidateId === candidateId);
    const existing = state.automationBlueprints?.find((item) => item.candidateId === candidateId);
    if (!intake || !existing) throw new BlueprintBridgeError("blueprint.not_found");
    if (existing.revision !== expectedRevision) throw new BlueprintBridgeError("blueprint.conflict", buildPreviewBlueprint(intake, existing, previewBlueprintEvidence(intake, state.checkIns, locale)));
    const reset = emptyBlueprintDraft(intake);
    const draft: AutomationBlueprintDraftV1 = { ...reset, revision: existing.revision + 1, createdAt: existing.createdAt, runsPerWeek: existing.runsPerWeek, dataSensitivity: existing.dataSensitivity, sideEffectLevel: existing.sideEffectLevel, verificationMode: existing.verificationMode, verificationMinutesPerRun: existing.verificationMinutesPerRun, recoveryPlan: existing.recoveryPlan };
    saveWebState({ ...state, automationBlueprints: [...(state.automationBlueprints ?? []).filter((item) => item.candidateId !== candidateId), draft] });
    return buildPreviewBlueprint(intake, draft, previewBlueprintEvidence(intake, state.checkIns, locale));
  },

  async exportAutomationBlueprint(candidateId: string, expectedRevision: number, format: "markdown" | "json", locale: string): Promise<string> {
    if (isTauri()) return invoke("export_automation_blueprint_v1", { candidateId, expectedRevision, format, locale });
    const blueprint = await bridge.getAutomationBlueprint(candidateId, locale);
    if (!blueprint || blueprint.status !== "ready") throw new BlueprintBridgeError("blueprint.not_ready");
    if (blueprint.draft.revision !== expectedRevision) throw new BlueprintBridgeError("blueprint.conflict", blueprint);
    return `preview://automation-blueprint-${candidateId}.${format === "markdown" ? "md" : "json"}`;
  },

  async deleteAutomationBlueprint(candidateId: string, expectedRevision: number, confirmation: string, locale: string): Promise<void> {
    if (isTauri()) return invoke("delete_automation_blueprint_v1", { candidateId, expectedRevision, confirmation, locale });
    if (confirmation !== "DELETE_AUTOMATION_BLUEPRINT") throw new BlueprintBridgeError("blueprint.delete_confirmation");
    const state = loadWebState();
    const current = state.automationBlueprints?.find((item) => item.candidateId === candidateId);
    if (!current) throw new BlueprintBridgeError("blueprint.not_found");
    if (current.revision !== expectedRevision) throw new BlueprintBridgeError("blueprint.conflict");
    // The UI only exposes a deletion-boundary preview; this stays available for a future explicit confirmation flow.
    saveWebState({ ...state, automationBlueprints: (state.automationBlueprints ?? []).filter((item) => item.candidateId !== candidateId) });
  },

  async pauseTracking(minutes: number): Promise<TrackingStatus> {
    if (isTauri()) return invoke("pause_tracking", { minutes });
    const state = loadWebState();
    state.pausedUntil = new Date(Date.now() + minutes * 60_000).toISOString();
    saveWebState(state);
    return bridge.getTrackingStatus();
  },

  async resumeTracking(): Promise<TrackingStatus> {
    if (isTauri()) return invoke("resume_tracking");
    const state = loadWebState();
    delete state.pausedUntil;
    saveWebState(state);
    return bridge.getTrackingStatus();
  },

  async exportData(format: "json" | "csv"): Promise<string> {
    if (isTauri()) return invoke("export_data", { format });
    return `preview://enso-time-export.${format}`;
  },

  async deleteAllData(): Promise<void> {
    if (isTauri()) return invoke("delete_all_data", { confirmation: "DELETE_ENSO_TIME_DATA" });
    localStorage.removeItem(STORAGE_KEY);
  },

  async checkForUpdate(): Promise<UpdateInfo> {
    return isTauri() ? invoke("check_for_update") : { available: false, currentVersion: __APP_VERSION__ };
  },

  async openExternal(url: string): Promise<void> {
    if (isTauri()) return invoke("open_external", { url });
    window.open(url, "_blank", "noopener,noreferrer");
  },
};
