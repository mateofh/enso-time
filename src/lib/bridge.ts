import { invoke } from "@tauri-apps/api/core";
import { demoCheckIns, demoDiagnostic, demoSettings, demoStatus, demoStudy, demoTimeline } from "./demo";
import { createDiagnostic } from "./diagnostics";
import type {
  CheckInAnswer,
  CheckInV1,
  DiagnosticV1,
  SettingsV1,
  StudyProgress,
  TimelineBlockV1,
  TrackingStatus,
  UpdateInfo,
} from "./types";

const STORAGE_KEY = "enso-time-web-state-v1";

interface WebState {
  settings: SettingsV1;
  checkIns: CheckInV1[];
  pausedUntil?: string;
}

function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

function loadWebState(): WebState {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) return JSON.parse(stored) as WebState;
  } catch {
    // A corrupt preview state should never prevent the UI from loading.
  }
  return { settings: demoSettings, checkIns: demoCheckIns };
}

function saveWebState(state: WebState) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
}

export const bridge = {
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
    return { ...demoStatus, running: !state.pausedUntil || new Date(state.pausedUntil) <= new Date(), pausedUntil: state.pausedUntil, pendingCheckIns: state.checkIns.filter((item) => item.status === "pending").length };
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
    return days === 7 ? createDiagnostic(demoTimeline, state.checkIns, 5, state.settings.role, state.settings.goals) : { ...demoDiagnostic, daysObserved: 5, preliminary: true };
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
    return isTauri() ? invoke("check_for_update") : { available: false, currentVersion: "0.1.0-beta.1" };
  },

  async openExternal(url: string): Promise<void> {
    if (isTauri()) return invoke("open_external", { url });
    window.open(url, "_blank", "noopener,noreferrer");
  },
};

