import { useEffect, useMemo, useState, type CSSProperties, type FormEvent } from "react";
import {
  Activity,
  AlertCircle,
  ArrowLeft,
  ArrowUpRight,
  Bot,
  Check,
  ChevronRight,
  Clipboard,
  Clock3,
  Download,
  Focus,
  Gauge,
  HelpCircle,
  Languages,
  LockKeyhole,
  Pause,
  Play,
  RefreshCw,
  Settings as SettingsIcon,
  Sparkles,
  Target,
  Trash2,
} from "lucide-react";
import { bridge } from "./lib/bridge";
import { WorkflowIntakeView } from "./components/WorkflowIntakeView";
import { AutomationBlueprintView } from "./components/AutomationBlueprintView";
import { CATEGORY_ORDER, categoryColor } from "./lib/categories";
import { categoryLabel, formatDuration, formatTime, messages } from "./lib/i18n";
import type {
  AutomationCandidateV1,
  CategoryId,
  CheckInAnswer,
  CheckInV1,
  CaptureIssue,
  DiagnosticV1,
  Locale,
  SettingsV1,
  StudyProgress,
  TimelineBlockV1,
  TrackingStatus,
  WorkflowIntakeV1,
} from "./lib/types";

type View = "today" | "checkins" | "study" | "automate" | "workflow" | "blueprint" | "settings";
type Notice = { kind: "success" | "error"; message: string };

const EMPTY_STATUS: TrackingStatus = {
  running: false,
  windowWatcher: "stopped",
  afkWatcher: "stopped",
  pendingCheckIns: 0,
  dataStaysLocal: true,
};

function BrandMark({ compact = false }: { compact?: boolean }) {
  return (
    <div className="brand" aria-label="Ensō Time">
      <span className="enso-mark" aria-hidden="true" />
      {!compact && <span className="wordmark">Ensō <b>Time</b></span>}
    </div>
  );
}

function StatusPill({ status, locale }: { status: TrackingStatus; locale: Locale }) {
  const copy = messages(locale).common;
  const state = status.captureIssue ? "is-degraded" : status.running ? "is-running" : "is-paused";
  return (
    <span className={`status-pill ${state}`}>
      <span className="status-dot" aria-hidden="true" />
      {status.captureIssue ? copy.degraded : status.running ? copy.running : copy.paused}
    </span>
  );
}

function CaptureIssueBanner({ issue, locale, onRetry }: { issue: CaptureIssue; locale: Locale; onRetry: () => Promise<void> }) {
  const copy = messages(locale).captureIssue;
  const [checking, setChecking] = useState(false);

  async function retry() {
    setChecking(true);
    try { await onRetry(); } finally { setChecking(false); }
  }

  return (
    <section className="capture-issue" aria-labelledby="capture-issue-title" aria-live="polite">
      <AlertCircle size={20} aria-hidden="true" />
      <div>
        <p>{copy.label}</p>
        <h2 id="capture-issue-title">{copy.title}</h2>
        <span>{copy.details[issue]}</span>
      </div>
      <button type="button" className="secondary-button" disabled={checking} onClick={() => void retry()}>
        <RefreshCw size={15} className={checking ? "is-spinning" : ""} aria-hidden="true" />{copy.retry}
      </button>
    </section>
  );
}

function Onboarding({ initial, onComplete }: { initial: SettingsV1; onComplete: (value: SettingsV1) => Promise<void> }) {
  const [step, setStep] = useState(0);
  const [role, setRole] = useState(initial.role);
  const [goal, setGoal] = useState(initial.goals[0] ?? "");
  const [locale, setLocale] = useState(initial.locale);
  const [captureTitles, setCaptureTitles] = useState(initial.captureWindowTitles);
  const [saving, setSaving] = useState(false);
  const copy = messages(locale).onboarding;
  const icons = [LockKeyhole, HelpCircle, Bot];
  const current = copy.steps[step] ?? copy.steps[0]!;
  const Icon = icons[step] ?? LockKeyhole;

  async function finish() {
    setSaving(true);
    try {
      await onComplete({
        ...initial,
        locale,
        captureWindowTitles: captureTitles,
        onboardingCompleted: true,
        role: role.trim(),
        goals: goal.trim() ? [goal.trim()] : initial.goals,
      });
    } finally {
      setSaving(false);
    }
  }

  return (
    <main className="onboarding-shell">
      <div className="onboarding-top">
        <BrandMark />
        <label className="language-picker">
          <span className="sr-only">{copy.roleLabel}</span>
          <Languages size={16} aria-hidden="true" />
          <select value={locale} onChange={(event) => setLocale(event.target.value as Locale)} aria-label="Language / Idioma">
            <option value="es-419">Español</option>
            <option value="en">English</option>
          </select>
        </label>
      </div>
      <section className="onboarding-panel" aria-labelledby="onboarding-title">
        <div className="onboarding-visual"><Icon size={38} strokeWidth={1.5} aria-hidden="true" /></div>
        <p className="page-context">{current.label}</p>
        <h1 id="onboarding-title">{current.title}</h1>
        <p className="lede">{current.copy}</p>
        {step === 2 && (
          <div className="onboarding-fields">
            <label>{copy.roleLabel}<input value={role} onChange={(event) => setRole(event.target.value)} placeholder={copy.rolePlaceholder} autoComplete="organization-title" /></label>
            <label>{copy.goalLabel}<input value={goal} onChange={(event) => setGoal(event.target.value)} placeholder={copy.goalPlaceholder} /></label>
            <label className="toggle-row onboarding-consent"><span><strong>{copy.titlesConsent}</strong><small>{copy.titlesConsentHint}</small></span><input type="checkbox" role="switch" checked={captureTitles} onChange={(event) => setCaptureTitles(event.target.checked)} /></label>
          </div>
        )}
        <div className="onboarding-actions">
          <div className="step-dots" aria-label={copy.progress(step + 1)}>
            {copy.steps.map((item, index) => <span key={item.label} className={index === step ? "active" : ""} aria-hidden="true" />)}
          </div>
          <div className="button-row">
            {step > 0 && <button type="button" className="text-button" onClick={() => setStep((value) => value - 1)}><ArrowLeft size={16} />{copy.back}</button>}
            <button type="button" className="primary-button" disabled={saving} onClick={() => step < 2 ? setStep((value) => value + 1) : void finish()}>
              {step < 2 ? copy.continue : copy.finish}<ChevronRight size={17} aria-hidden="true" />
            </button>
          </div>
        </div>
      </section>
    </main>
  );
}

function Timeline({ blocks, locale }: { blocks: TimelineBlockV1[]; locale: Locale }) {
  const copy = messages(locale).today;
  if (blocks.length === 0) return <p className="inline-empty">{copy.emptyTimeline}</p>;

  return (
    <div className="timeline" aria-label={copy.timeline}>
      {blocks.map((block) => (
        <article className="timeline-row" key={block.id}>
          <time dateTime={block.start}>{formatTime(block.start, locale)}</time>
          <span className="timeline-rail" style={{ "--category-color": categoryColor(block.category) } as CSSProperties} aria-hidden="true" />
          <div className="timeline-copy">
            <div><strong>{block.app}</strong><span>{formatDuration(block.durationSeconds, locale)}</span></div>
            <p title={block.title}>{block.title || categoryLabel(locale, block.category)}</p>
          </div>
          <span className="category-tag"><i style={{ background: categoryColor(block.category) }} aria-hidden="true" />{categoryLabel(locale, block.category)}</span>
        </article>
      ))}
    </div>
  );
}

function TodayView({ settings, status, blocks, checkIns, onNavigate, onPause, onResume }: {
  settings: SettingsV1;
  status: TrackingStatus;
  blocks: TimelineBlockV1[];
  checkIns: CheckInV1[];
  onNavigate: (view: View) => void;
  onPause: () => Promise<void>;
  onResume: () => Promise<void>;
}) {
  const locale = settings.locale;
  const copy = messages(locale).today;
  const totals = useMemo(() => {
    const values = new Map<CategoryId, number>();
    blocks.forEach((block) => values.set(block.category, (values.get(block.category) ?? 0) + block.durationSeconds));
    return CATEGORY_ORDER.map((category) => ({ category, seconds: values.get(category) ?? 0 })).filter((item) => item.seconds > 0);
  }, [blocks]);
  const totalSeconds = totals.reduce((sum, item) => sum + item.seconds, 0);
  const pending = checkIns.find((item) => item.status === "pending");
  const date = new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long" }).format(new Date());

  return (
    <>
      <header className="page-header">
        <div><p className="page-context">{copy.dateLabel} · {date}</p><h1>{copy.title}</h1><p className="page-intro">{copy.observed(formatDuration(totalSeconds, locale))}</p></div>
        <button type="button" className="secondary-button" onClick={() => void (status.running ? onPause() : onResume())}>
          {status.running ? <Pause size={16} aria-hidden="true" /> : <Play size={16} aria-hidden="true" />}{status.running ? copy.pause : copy.resume}
        </button>
      </header>

      {pending && (
        <button type="button" className="context-callout" onClick={() => onNavigate("checkins")}>
          <span className="callout-icon"><HelpCircle size={19} aria-hidden="true" /></span>
          <span><strong>{copy.missingTitle}</strong><small>{copy.missingCopy(formatTime(pending.intervalStart, locale), formatTime(pending.intervalEnd, locale))}</small></span>
          <ChevronRight size={18} aria-hidden="true" />
        </button>
      )}

      <div className="today-layout">
        <section className="section-block" aria-labelledby="timeline-title">
          <div className="section-heading"><div><p className="section-note">{copy.timelineHint}</p><h2 id="timeline-title">{copy.timeline}</h2></div><StatusPill status={status} locale={locale} /></div>
          <Timeline blocks={blocks} locale={locale} />
        </section>
        <aside className="day-summary" aria-labelledby="distribution-title">
          <p className="section-note">{copy.distributionHint}</p>
          <h2 id="distribution-title">{copy.distribution}</h2>
          {totals.length > 0 ? (
            <div className="distribution-list">
              {totals.map((item) => (
                <div className="distribution-item" key={item.category}>
                  <div><span><i style={{ background: categoryColor(item.category) }} aria-hidden="true" />{categoryLabel(locale, item.category)}</span><b>{formatDuration(item.seconds, locale)}</b></div>
                  <span className="bar" role="progressbar" aria-label={categoryLabel(locale, item.category)} aria-valuemin={0} aria-valuemax={totalSeconds} aria-valuenow={item.seconds}><i style={{ width: `${Math.max(4, item.seconds / totalSeconds * 100)}%`, background: categoryColor(item.category) }} /></span>
                </div>
              ))}
            </div>
          ) : <p className="inline-empty">{copy.emptyTimeline}</p>}
          <div className="privacy-note"><LockKeyhole size={16} aria-hidden="true" /><span><strong>{copy.privateTitle}</strong>{copy.privateCopy}</span></div>
        </aside>
      </div>
    </>
  );
}

function CheckInsView({ checkIns, settings, onAnswer }: { checkIns: CheckInV1[]; settings: SettingsV1; onAnswer: (value: CheckInAnswer) => Promise<void> }) {
  const pending = checkIns.filter((item) => item.status === "pending");
  const history = checkIns.filter((item) => item.status === "answered");
  const active = pending[0];
  const copy = messages(settings.locale).checkins;
  const [activityLabel, setActivityLabel] = useState("");
  const [category, setCategory] = useState<CategoryId>("admin");
  const [project, setProject] = useState("");
  const [repetitive, setRepetitive] = useState(false);
  const [rulesClear, setRulesClear] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setActivityLabel(""); setProject(""); setRepetitive(false); setRulesClear(false);
  }, [active?.id]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!active || !activityLabel.trim()) return;
    setSaving(true);
    try {
      await onAnswer({ id: active.id, activityLabel: activityLabel.trim(), category, project: project.trim() || undefined, repetitive, rulesClear });
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      <header className="page-header"><div><p className="page-context">{copy.pending(pending.length)}</p><h1>{copy.title}</h1><p className="page-intro">{copy.intro}</p></div></header>
      {active ? (
        <form className="checkin-form" onSubmit={(event) => void submit(event)}>
          <div className="interval-label"><Clock3 size={18} aria-hidden="true" /><span><strong>{formatTime(active.intervalStart, settings.locale)}–{formatTime(active.intervalEnd, settings.locale)}</strong><small>{active.reason === "afk" ? copy.afk : copy.ambiguous}</small></span></div>
          <label className="field-label">{copy.activity}<input autoFocus value={activityLabel} onChange={(event) => setActivityLabel(event.target.value)} placeholder={copy.activityPlaceholder} /></label>
          <div className="form-grid">
            <label className="field-label">{copy.category}<select value={category} onChange={(event) => setCategory(event.target.value as CategoryId)}>{CATEGORY_ORDER.map((item) => <option key={item} value={item}>{categoryLabel(settings.locale, item)}</option>)}</select></label>
            <label className="field-label">{copy.project}<input value={project} onChange={(event) => setProject(event.target.value)} placeholder={copy.projectPlaceholder} /></label>
          </div>
          <label className="check-row"><input type="checkbox" checked={repetitive} onChange={(event) => setRepetitive(event.target.checked)} /><span><strong>{copy.repetitive}</strong><small>{copy.repetitiveHint}</small></span></label>
          <label className="check-row"><input type="checkbox" checked={rulesClear} onChange={(event) => setRulesClear(event.target.checked)} /><span><strong>{copy.rules}</strong><small>{copy.rulesHint}</small></span></label>
          <button className="primary-button" disabled={!activityLabel.trim() || saving}><Check size={17} aria-hidden="true" />{copy.save}</button>
        </form>
      ) : (
        <section className="empty-state"><Check size={30} aria-hidden="true" /><h2>{copy.allDone}</h2><p>{copy.allDoneCopy}</p></section>
      )}
      {history.length > 0 && <section className="history-section" aria-labelledby="history-title"><div className="section-heading"><h2 id="history-title">{copy.history}</h2></div>{history.slice(0, 5).map((item) => <div className="history-row" key={item.id}><span><i style={{ background: categoryColor(item.category ?? "unknown") }} aria-hidden="true" /><strong>{item.activityLabel}</strong><small>{item.project || categoryLabel(settings.locale, item.category ?? "unknown")}</small></span><time dateTime={item.answeredAt}>{item.answeredAt ? formatTime(item.answeredAt, settings.locale) : "—"}</time></div>)}</section>}
    </>
  );
}

function StudyView({ progress, locale, onDays }: { progress: StudyProgress; locale: Locale; onDays: (days: 7 | 14 | 28) => void }) {
  const copy = messages(locale).study;
  const circumference = 2 * Math.PI * 54;
  const offset = circumference - circumference * Math.min(progress.coveragePercent, 100) / 100;
  const observed = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(progress.observedDays);
  const hours = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(progress.trackedHours);
  const remaining = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(Math.max(0, 7 - progress.observedDays));
  return (
    <>
      <header className="page-header"><div><p className="page-context">{progress.observedDays >= 7 ? copy.available : copy.preliminary}</p><h1>{copy.title}</h1><p className="page-intro">{copy.intro}</p></div></header>
      <section className="study-hero">
        <div className="progress-ring"><svg viewBox="0 0 128 128" role="img" aria-label={`${progress.coveragePercent}% ${copy.coverage}`}><circle cx="64" cy="64" r="54" /><circle className="progress-value" cx="64" cy="64" r="54" style={{ strokeDasharray: circumference, strokeDashoffset: offset }} /></svg><span><b>{observed}</b><small>{locale === "en" ? `of ${progress.selectedDays} ${copy.days}` : `de ${progress.selectedDays} ${copy.days}`}</small></span></div>
        <div className="study-copy"><h2>{progress.observedDays >= 7 ? copy.usefulSignal : copy.remaining(remaining)}</h2><p>{copy.evidence(hours, progress.answeredCheckIns)}</p><div className="study-metrics"><span><b>{progress.coveragePercent}%</b>{copy.coverage}</span><span><b>{hours} h</b>{copy.tracked}</span><span><b>{progress.answeredCheckIns}</b>{copy.answers}</span></div></div>
      </section>
      <section className="period-section"><h2>{copy.periodTitle}</h2><div className="segmented-control" aria-label={copy.periodTitle}>{([7, 14, 28] as const).map((days) => <button type="button" key={days} className={progress.selectedDays === days ? "active" : ""} aria-pressed={progress.selectedDays === days} onClick={() => onDays(days)}><b>{days} {copy.days}</b><small>{copy.periods[days]}</small></button>)}</div></section>
    </>
  );
}

function AutomateView({ diagnostic, locale, busy, onGenerate, onCopy, onOpen, onWorkflow, blueprintCandidate }: { diagnostic: DiagnosticV1; locale: Locale; busy: boolean; onGenerate: () => Promise<void>; onCopy: () => Promise<void>; onOpen: (url: string) => Promise<void>; onWorkflow: (candidate: AutomationCandidateV1) => void; blueprintCandidate: AutomationCandidateV1 | null }) {
  const copy = messages(locale).automate;
  const candidate = diagnostic.candidates[0];
  return (
    <>
      <header className="page-header"><div><p className="page-context">{copy.candidate}</p><h1>{copy.title}</h1><p className="page-intro">{copy.intro}</p></div><button type="button" className="secondary-button" disabled={busy} onClick={() => void onGenerate()}><RefreshCw size={16} aria-hidden="true" />{copy.refresh}</button></header>
      {diagnostic.preliminary && <div className="preliminary-banner"><Gauge size={18} aria-hidden="true" /><span><strong>{copy.preliminaryTitle}</strong>{copy.preliminaryCopy(diagnostic.daysObserved.toFixed(1))}</span></div>}
      {candidate ? (
        <section className="candidate-section" aria-labelledby="candidate-title">
          <div className="candidate-rank"><span>{candidate.priorityScore}</span><small>/ 100</small></div>
          <div className="candidate-main"><p className="section-note">{copy.candidate}</p><h2 id="candidate-title">{candidate.activityLabel}</h2><p>{candidate.evidence.join(" · ")}</p><div className="score-line"><span><b>{candidate.priorityScore}</b>/100 {copy.signal}</span><span><b>{candidate.estimatedWeeklyHours}</b> {copy.weeklyHours}</span><span><b>{candidate.weeklyFrequency}</b> {copy.repetitions}</span></div><div className="workflow-cta"><button type="button" className="secondary-button" onClick={() => onWorkflow(candidate)}>{copy.workflowAction}<ChevronRight size={16} aria-hidden="true" /></button><small>{copy.workflowHint}</small></div></div>
          <div className="candidate-scores"><span><b>{candidate.impactScore}/40</b>{copy.impact}</span><span><b>{candidate.feasibilityScore}/35</b>{copy.feasibility}</span><span><b>{candidate.riskScore}/25</b>{copy.risk}</span></div>
        </section>
      ) : <section className="empty-state"><Target size={30} aria-hidden="true" /><h2>{copy.empty}</h2><p>{copy.emptyCopy}</p></section>}
      <section className="recommendations"><h2>{copy.recommendations}</h2><ol>{diagnostic.recommendations.map((recommendation, index) => <li key={recommendation}><span>{String(index + 1).padStart(2, "0")}</span><p>{recommendation}</p></li>)}</ol></section>
      <section className="prompt-section" aria-labelledby="prompt-title">
        <div className="prompt-heading"><div><p className="section-note">{copy.promptIntro}</p><h2 id="prompt-title">{copy.promptTitle}</h2></div><button type="button" className="primary-button" onClick={() => void onCopy()}><Clipboard size={16} aria-hidden="true" />{copy.copyPrompt}</button></div>
        <pre tabIndex={0}>{diagnostic.prompt}</pre>
        <div className="prompt-links"><button type="button" onClick={() => void onOpen("https://chatgpt.com/")}>{copy.openChatGpt} <ArrowUpRight size={15} aria-hidden="true" /></button><button type="button" onClick={() => void onOpen("https://claude.ai/new")}>{copy.openClaude} <ArrowUpRight size={15} aria-hidden="true" /></button><span><LockKeyhole size={14} aria-hidden="true" />{copy.outboundNotice}</span></div>
      </section>
      {blueprintCandidate ? <section className="blueprint-entry-card" aria-labelledby="blueprint-entry-title"><div><p className="section-note">{messages(locale).blueprint.context}</p><h2 id="blueprint-entry-title">{blueprintCandidate.activityLabel}</h2><p>{messages(locale).blueprint.intro}</p></div><button type="button" className="primary-button" data-testid="blueprint-entry" onClick={() => onWorkflow(blueprintCandidate)}>{messages(locale).blueprint.mapAction}<ChevronRight size={16} aria-hidden="true" /></button></section> : <section className="empty-state blueprint-no-candidate" data-testid="blueprint-no-candidate"><Target size={30} aria-hidden="true" /><h2>{copy.empty}</h2><p>{copy.emptyCopy}</p></section>}
    </>
  );
}

function SettingsView({ settings, status, onSave, onExport, onDelete }: { settings: SettingsV1; status: TrackingStatus; onSave: (value: SettingsV1) => Promise<void>; onExport: (format: "json" | "csv") => Promise<void>; onDelete: () => Promise<void> }) {
  const [draft, setDraft] = useState(settings);
  const [saving, setSaving] = useState(false);
  const copy = messages(draft.locale).settings;
  useEffect(() => setDraft(settings), [settings]);
  const set = <K extends keyof SettingsV1>(key: K, value: SettingsV1[K]) => setDraft((current) => ({ ...current, [key]: value }));
  const dirty = JSON.stringify(draft) !== JSON.stringify(settings);

  async function save() {
    setSaving(true);
    try { await onSave(draft); } finally { setSaving(false); }
  }

  return (
    <>
      <header className="page-header"><div><p className="page-context">{messages(draft.locale).common.local}</p><h1>{copy.title}</h1><p className="page-intro">{copy.intro}</p></div><button type="button" className="primary-button" disabled={!dirty || saving} onClick={() => void save()}><Check size={16} aria-hidden="true" />{copy.save}</button></header>
      <section className="settings-group"><div className="settings-intro"><Clock3 size={19} aria-hidden="true" /><div><h2>{copy.tracking}</h2><p>{copy.trackingHint}</p></div></div><div className="settings-fields"><div className="two-columns"><label>{copy.cadence}<select value={draft.cadenceMinutes} onChange={(event) => set("cadenceMinutes", Number(event.target.value) as 15 | 30 | 60)}><option value={15}>{copy.cadenceOption(15)}</option><option value={30}>{copy.cadenceOption(30)}</option><option value={60}>{copy.cadenceOption(60)}</option></select></label><label>{copy.retention}<select value={draft.retentionDays ?? "forever"} onChange={(event) => set("retentionDays", event.target.value === "forever" ? null : Number(event.target.value) as 30 | 90 | 365)}><option value={30}>{copy.retention30}</option><option value={90}>{copy.retention90}</option><option value={365}>{copy.retention365}</option><option value="forever">{copy.retentionForever}</option></select></label></div><div><p className="field-group-title">{copy.schedule}</p><p className="field-group-help">{copy.scheduleHint}</p><div className="two-columns"><label>{copy.start}<input type="time" value={draft.workSchedule.start} onChange={(event) => set("workSchedule", { ...draft.workSchedule, start: event.target.value })} /></label><label>{copy.end}<input type="time" value={draft.workSchedule.end} onChange={(event) => set("workSchedule", { ...draft.workSchedule, end: event.target.value })} /></label></div></div><label className="toggle-row"><span><strong>{copy.autostart}</strong><small>{copy.autostartHint}</small></span><input type="checkbox" role="switch" checked={draft.autostart} onChange={(event) => set("autostart", event.target.checked)} /></label><label className="toggle-row"><span><strong>{copy.titles}</strong><small>{copy.titlesHint}</small></span><input type="checkbox" role="switch" checked={draft.captureWindowTitles} onChange={(event) => set("captureWindowTitles", event.target.checked)} /></label></div></section>
      <section className="settings-group"><div className="settings-intro"><Languages size={19} aria-hidden="true" /><div><h2>{copy.appearance}</h2><p>{copy.appearanceHint}</p></div></div><div className="settings-fields"><div className="two-columns"><label>{copy.language}<select value={draft.locale} onChange={(event) => set("locale", event.target.value as Locale)}><option value="es-419">Español (Latinoamérica)</option><option value="en">English</option></select></label><label>{copy.theme}<select value={draft.theme} onChange={(event) => set("theme", event.target.value as SettingsV1["theme"])}><option value="ink">{copy.ink}</option><option value="paper">{copy.paper}</option><option value="system">{copy.system}</option></select></label></div><label className="toggle-row"><span><strong>{copy.updates}</strong><small>{copy.updatesHint}</small></span><input type="checkbox" role="switch" checked={draft.updateChecksEnabled} onChange={(event) => set("updateChecksEnabled", event.target.checked)} /></label></div></section>
      <section className="settings-group"><div className="settings-intro"><LockKeyhole size={19} aria-hidden="true" /><div><h2>{copy.data}</h2><p>{copy.localEngine(status.serverPort)}</p></div></div><div className="data-actions"><button type="button" className="secondary-button" onClick={() => void onExport("json")}><Download size={16} aria-hidden="true" />{copy.exportJson}</button><button type="button" className="secondary-button" onClick={() => void onExport("csv")}><Download size={16} aria-hidden="true" />{copy.exportCsv}</button><button type="button" className="danger-button" onClick={() => void onDelete()}><Trash2 size={16} aria-hidden="true" />{copy.delete}</button></div></section>
    </>
  );
}

export default function App() {
  const [view, setView] = useState<View>("today");
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string>();
  const [settings, setSettings] = useState<SettingsV1 | null>(null);
  const [status, setStatus] = useState<TrackingStatus>(EMPTY_STATUS);
  const [blocks, setBlocks] = useState<TimelineBlockV1[]>([]);
  const [checkIns, setCheckIns] = useState<CheckInV1[]>([]);
  const [study, setStudy] = useState<StudyProgress | null>(null);
  const [diagnostic, setDiagnostic] = useState<DiagnosticV1 | null>(null);
  const [notice, setNotice] = useState<Notice>();
  const [diagnosticBusy, setDiagnosticBusy] = useState(false);
  const [workflowCandidate, setWorkflowCandidate] = useState<AutomationCandidateV1 | null>(null);
  const [workflowIntakes, setWorkflowIntakes] = useState<WorkflowIntakeV1[]>([]);

  async function load() {
    setLoading(true);
    setLoadError(undefined);
    try {
      const from = new Date(); from.setHours(0, 0, 0, 0);
      const to = new Date(); to.setHours(23, 59, 59, 999);
      const [nextSettings, nextStatus, nextBlocks, nextCheckIns, nextStudy, nextWorkflowIntakes] = await Promise.all([
        bridge.getSettings(), bridge.getTrackingStatus(), bridge.getTimeline(from.toISOString(), to.toISOString()), bridge.listCheckIns(), bridge.getStudyProgress(), bridge.listWorkflowIntakes(),
      ]);
      setSettings(nextSettings); setStatus(nextStatus); setBlocks(nextBlocks); setCheckIns(nextCheckIns); setStudy(nextStudy); setWorkflowIntakes(nextWorkflowIntakes);
      setDiagnostic(await bridge.generateDiagnostic(nextStudy.selectedDays));
      if (bridge.shouldResumeAutomationBlueprint()) {
        const intake = nextWorkflowIntakes[0];
        if (intake) {
          setWorkflowCandidate({ id: intake.candidateId, activityLabel: intake.candidateLabel, estimatedWeeklyHours: 0, weeklyFrequency: 0, impactScore: 0, feasibilityScore: 0, riskScore: 0, priorityScore: 0, evidence: [] });
          setView("blueprint");
        }
      }
    } catch (error) {
      setLoadError(error instanceof Error ? error.message : "Unable to load Ensō Time");
    } finally {
      setLoading(false);
    }
  }

  async function action(run: () => Promise<void>, success?: string) {
    const fallback = settings ? messages(settings.locale).common.unexpectedError : "No pudimos completar la acción.";
    try {
      await run();
      if (success) setNotice({ kind: "success", message: success });
    } catch (error) {
      setNotice({ kind: "error", message: error instanceof Error ? error.message : fallback });
    }
  }

  useEffect(() => { void load(); }, []);
  useEffect(() => {
    if (!settings) return;
    const media = matchMedia("(prefers-color-scheme: light)");
    const applyTheme = () => {
      document.documentElement.dataset.theme = settings.theme === "system" ? (media.matches ? "paper" : "ink") : settings.theme;
      document.documentElement.lang = settings.locale;
    };
    applyTheme();
    media.addEventListener("change", applyTheme);
    return () => media.removeEventListener("change", applyTheme);
  }, [settings]);
  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(undefined), 3200);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const loadingCopy = settings ? messages(settings.locale).common : messages("es-419").common;
  if (loading) return <main className="loading-screen" aria-busy="true"><BrandMark /><span className="loading-line" /><p>{loadingCopy.loading}</p></main>;
  if (loadError || !settings || !study || !diagnostic) return <main className="error-screen"><AlertCircle size={30} aria-hidden="true" /><h1>Ensō Time</h1><p>{loadError || loadingCopy.unexpectedError}</p><button type="button" className="primary-button" onClick={() => void load()}>{loadingCopy.retry}</button></main>;
  if (!settings.onboardingCompleted) return <Onboarding initial={settings} onComplete={async (value) => { setSettings(await bridge.saveSettings(value)); }} />;

  const copy = messages(settings.locale);
  const items: { id: View; label: string; icon: typeof Activity }[] = [
    { id: "today", label: copy.nav.today, icon: Activity },
    { id: "checkins", label: copy.nav.checkins, icon: HelpCircle },
    { id: "study", label: copy.nav.study, icon: Focus },
    { id: "automate", label: copy.nav.automate, icon: Sparkles },
    { id: "settings", label: copy.nav.settings, icon: SettingsIcon },
  ];
  const saveSettings = async (value: SettingsV1) => action(async () => { setSettings(await bridge.saveSettings(value)); }, messages(value.locale).notices.settingsSaved);
  const answer = async (value: CheckInAnswer) => action(async () => { await bridge.answerCheckIn(value); setCheckIns(await bridge.listCheckIns()); setStatus(await bridge.getTrackingStatus()); }, copy.notices.contextSaved);
  const selectDays = (days: 7 | 14 | 28) => {
    setStudy({ ...study, selectedDays: days });
    setDiagnosticBusy(true);
    void action(async () => { setDiagnostic(await bridge.generateDiagnostic(days)); }).finally(() => setDiagnosticBusy(false));
  };
  const saveWorkflowIntake = async (intake: WorkflowIntakeV1) => {
    const saved = await bridge.saveWorkflowIntake(intake);
    setWorkflowIntakes(await bridge.listWorkflowIntakes());
    setDiagnostic(await bridge.generateDiagnostic(study.selectedDays));
    setNotice({ kind: "success", message: copy.workflow.savedTitle });
    return saved;
  };
  const blueprintCandidate = workflowIntakes[0];
  const blueprintEntry: AutomationCandidateV1 | null = blueprintCandidate ? { id: blueprintCandidate.candidateId, activityLabel: blueprintCandidate.candidateLabel, estimatedWeeklyHours: 0, weeklyFrequency: 0, impactScore: 0, feasibilityScore: 0, riskScore: 0, priorityScore: 0, evidence: [] } : null;

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">{copy.nav.today}</a>
      <aside className="sidebar">
        <BrandMark />
        <nav aria-label="Principal">{items.map(({ id, label, icon: Icon }) => <button type="button" key={id} className={view === id ? "active" : ""} aria-current={view === id ? "page" : undefined} aria-label={label} title={label} onClick={() => setView(id)}><Icon size={18} aria-hidden="true" /><span>{label}</span>{id === "checkins" && status.pendingCheckIns > 0 && <b aria-label={`${status.pendingCheckIns}`}>{status.pendingCheckIns}</b>}</button>)}</nav>
        <div className="sidebar-footer"><StatusPill status={status} locale={settings.locale} /><p><LockKeyhole size={13} aria-hidden="true" />{copy.common.local}</p><small>Ensō Time v0.1 beta</small></div>
      </aside>
      <main className="content" id="main-content" tabIndex={-1}>
        {status.captureIssue && <CaptureIssueBanner issue={status.captureIssue} locale={settings.locale} onRetry={() => action(async () => { setStatus(await bridge.getTrackingStatus()); })} />}
        {view === "today" && <TodayView settings={settings} status={status} blocks={blocks} checkIns={checkIns} onNavigate={setView} onPause={() => action(async () => { setStatus(await bridge.pauseTracking(30)); }, copy.notices.paused)} onResume={() => action(async () => { setStatus(await bridge.resumeTracking()); }, copy.notices.resumed)} />}
        {view === "checkins" && <CheckInsView checkIns={checkIns} settings={settings} onAnswer={answer} />}
        {view === "study" && <StudyView progress={study} locale={settings.locale} onDays={selectDays} />}
        {view === "automate" && <AutomateView diagnostic={diagnostic} locale={settings.locale} busy={diagnosticBusy} onGenerate={async () => { setDiagnosticBusy(true); await action(async () => { setDiagnostic(await bridge.generateDiagnostic(study.selectedDays)); }, copy.notices.diagnosticUpdated); setDiagnosticBusy(false); }} onCopy={() => action(async () => { await navigator.clipboard.writeText(diagnostic.prompt); }, copy.notices.promptCopied)} onOpen={(url) => action(() => bridge.openExternal(url))} onWorkflow={(candidate) => { setWorkflowCandidate(candidate); setView(candidate.id === blueprintEntry?.id ? "blueprint" : "workflow"); }} blueprintCandidate={blueprintEntry} />}
        {view === "workflow" && <WorkflowIntakeView candidate={workflowCandidate} locale={settings.locale} onBack={() => setView("automate")} onSaved={saveWorkflowIntake} />}
        {view === "blueprint" && <AutomationBlueprintView candidate={workflowCandidate} locale={settings.locale} onBack={() => setView("automate")} onMapWorkflow={(candidate) => { setWorkflowCandidate(candidate); setView("workflow"); }} />}
        {view === "settings" && <SettingsView settings={settings} status={status} onSave={saveSettings} onExport={(format) => action(async () => { const path = await bridge.exportData(format); setNotice({ kind: "success", message: copy.notices.exported(path) }); })} onDelete={() => action(async () => { if (!window.confirm(copy.settings.deleteConfirm)) return; await bridge.deleteAllData(); setNotice({ kind: "success", message: copy.notices.deleted }); await load(); })} />}
      </main>
      {notice && <div className={`toast ${notice.kind === "error" ? "is-error" : ""}`} role={notice.kind === "error" ? "alert" : "status"}>{notice.kind === "error" ? <AlertCircle size={15} aria-hidden="true" /> : <Check size={15} aria-hidden="true" />}{notice.message}</div>}
    </div>
  );
}
