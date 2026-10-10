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
  ListChecks,
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
  ClassificationRuleV1,
  CheckInAnswer,
  CheckInV1,
  CaptureIssue,
  DiagnosticV1,
  Locale,
  RuleImpactV1,
  ReviewGroupV1,
  ReviewOperationV1,
  SessionClassificationInputV1,
  SessionClassificationResultV1,
  SettingsV1,
  StudyProgress,
  TimelineBlockV1,
  TrackingStatus,
  WorkflowIntakeV1,
  AiConfigurationStatusV1,
  ConnectorClientV1,
  ConnectorPairingV1,
  EvaluationPayloadPreviewV1,
  EvaluationReportV1,
  OpenRouterModelV1,
  ScreenOcrStatusV1,
} from "./lib/types";

type View = "today" | "review" | "checkins" | "study" | "automate" | "workflow" | "blueprint" | "settings";
type Notice = { kind: "success" | "error"; message: string };

const EMPTY_STATUS: TrackingStatus = {
  running: false,
  windowWatcher: "stopped",
  afkWatcher: "stopped",
  pendingCheckIns: 0,
  dataStaysLocal: true,
  processingMode: "local-only",
  experimentalIntelligence: false,
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

function CaptureIssueBanner({ issue, locale, onRetry, onRequestAccessibility }: { issue: CaptureIssue; locale: Locale; onRetry: () => Promise<void>; onRequestAccessibility: () => Promise<void> }) {
  const copy = messages(locale).captureIssue;
  const [checking, setChecking] = useState(false);
  const [opening, setOpening] = useState(false);

  async function retry() {
    setChecking(true);
    try { await onRetry(); } finally { setChecking(false); }
  }

  async function requestAccessibility() {
    setOpening(true);
    try { await onRequestAccessibility(); } finally { setOpening(false); }
  }

  return (
    <section className="capture-issue" aria-labelledby="capture-issue-title" aria-live="polite">
      <AlertCircle size={20} aria-hidden="true" />
      <div>
        <p>{copy.label}</p>
        <h2 id="capture-issue-title">{copy.title}</h2>
        <span>{copy.details[issue]}</span>
      </div>
      <div className="capture-issue-actions">
        {issue === "accessibility-permission-required" && <button type="button" className="secondary-button" disabled={opening || checking} onClick={() => void requestAccessibility()}><SettingsIcon size={15} aria-hidden="true" />{copy.accessibilityAction}</button>}
        <button type="button" className="secondary-button" disabled={checking || opening} onClick={() => void retry()}>
          <RefreshCw size={15} className={checking ? "is-spinning" : ""} aria-hidden="true" />{copy.retry}
        </button>
      </div>
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

const RULE_CATEGORIES = CATEGORY_ORDER.filter((category) => !["unknown", "break"].includes(category));
const SESSION_CATEGORIES = CATEGORY_ORDER.filter((category) => category !== "unknown");

function Timeline({ blocks, locale, onClassify, onClear, onSaveRule, onPreviewRule }: {
  blocks: TimelineBlockV1[];
  locale: Locale;
  onClassify: (input: SessionClassificationInputV1) => Promise<SessionClassificationResultV1>;
  onClear: (blockId: string) => Promise<void>;
  onSaveRule: (rule: ClassificationRuleV1) => Promise<void>;
  onPreviewRule: (rule: ClassificationRuleV1) => Promise<RuleImpactV1>;
}) {
  const copy = messages(locale).today;
  const classificationCopy = messages(locale).classification;
  const [editing, setEditing] = useState<string>();
  const [category, setCategory] = useState<CategoryId>("admin");
  const [activityLabel, setActivityLabel] = useState("");
  const [suggestion, setSuggestion] = useState<SessionClassificationResultV1["suggestion"]>();
  const [ruleDraft, setRuleDraft] = useState<ClassificationRuleV1>();
  const [impact, setImpact] = useState<RuleImpactV1>();
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>();
  if (blocks.length === 0) return <p className="inline-empty">{copy.emptyTimeline}</p>;

  function openEditor(block: TimelineBlockV1) {
    setEditing(block.id);
    setCategory(block.category === "unknown" || block.category === "break" ? "admin" : block.category);
    setActivityLabel(block.activityLabel ?? "");
    setSuggestion(undefined);
    setRuleDraft(undefined);
    setImpact(undefined);
    setError(undefined);
  }

  async function saveSession(event: FormEvent) {
    event.preventDefault();
    if (!editing) return;
    setSaving(true); setError(undefined);
    try {
      const result = await onClassify({ blockId: editing, category, activityLabel: activityLabel.trim() || undefined });
      setSuggestion(result.suggestion);
      setRuleDraft(result.suggestion?.rule);
      setImpact(result.suggestion?.impact);
      if (!result.suggestion) setEditing(undefined);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : messages(locale).common.unexpectedError);
    } finally { setSaving(false); }
  }

  async function refreshImpact() {
    if (!ruleDraft) return;
    try { setImpact(await onPreviewRule(ruleDraft)); } catch { /* Save will surface validation errors. */ }
  }

  async function createRule() {
    if (!ruleDraft) return;
    setSaving(true); setError(undefined);
    try {
      await onSaveRule(ruleDraft);
      setEditing(undefined); setSuggestion(undefined); setRuleDraft(undefined);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : messages(locale).common.unexpectedError);
    } finally { setSaving(false); }
  }

  async function restoreSession(blockId: string) {
    setSaving(true); setError(undefined);
    try { await onClear(blockId); setEditing(undefined); }
    catch (caught) { setError(caught instanceof Error ? caught.message : messages(locale).common.unexpectedError); }
    finally { setSaving(false); }
  }

  return (
    <div className="timeline" aria-label={copy.timeline}>
      {blocks.map((block) => (
        <article className="timeline-row" key={block.id}>
          <time dateTime={block.start}>{formatTime(block.start, locale)}</time>
          <span className="timeline-rail" style={{ "--category-color": categoryColor(block.category) } as CSSProperties} aria-hidden="true" />
          <div className="timeline-copy">
            <div><strong>{block.app}</strong><span>{formatDuration(block.durationSeconds, locale)}</span></div>
            <p title={block.title}>{block.activityLabel || block.title || categoryLabel(locale, block.category)}</p>
          </div>
          {block.correctable !== false ? <button type="button" className="category-tag category-button" aria-label={`${classificationCopy.edit}: ${categoryLabel(locale, block.category)}`} onClick={() => openEditor(block)}><i style={{ background: categoryColor(block.category) }} aria-hidden="true" />{categoryLabel(locale, block.category)}</button> : <span className="category-tag"><i style={{ background: categoryColor(block.category) }} aria-hidden="true" />{categoryLabel(locale, block.category)}</span>}
          {editing === block.id && (
            <form className="classification-editor" onSubmit={(event) => void saveSession(event)}>
              {!suggestion ? <>
                <div><strong>{classificationCopy.editorTitle}</strong><small>{block.app}{block.title ? ` · ${block.title}` : ""}</small></div>
                <div className="classification-fields">
                  <label>{classificationCopy.category}<select value={category} onChange={(event) => setCategory(event.target.value as CategoryId)}>{SESSION_CATEGORIES.map((item) => <option key={item} value={item}>{categoryLabel(locale, item)}</option>)}</select></label>
                  <label>{classificationCopy.activityLabel}<input value={activityLabel} maxLength={200} onChange={(event) => setActivityLabel(event.target.value)} placeholder={classificationCopy.activityPlaceholder} /></label>
                </div>
                {error && <p className="form-error" role="alert">{error}</p>}
                <div className="button-row"><button type="submit" className="primary-button" disabled={saving}><Check size={16} />{classificationCopy.saveSession}</button>{block.classificationSource === "override" && <button type="button" className="text-button" disabled={saving} onClick={() => void restoreSession(block.id)}><RefreshCw size={15} />{classificationCopy.restore}</button>}<button type="button" className="text-button" onClick={() => setEditing(undefined)}>{classificationCopy.cancel}</button></div>
              </> : ruleDraft && <>
                <div><strong>{classificationCopy.suggestionTitle}</strong><small>{classificationCopy.suggestionCopy}</small></div>
                <div className="classification-fields">
                  <label>{classificationCopy.scope}<select value={ruleDraft.scope} onChange={(event) => setRuleDraft({ ...ruleDraft, scope: event.target.value as "app" | "app-title", titlePattern: event.target.value === "app" ? undefined : ruleDraft.titlePattern ?? block.title ?? "" })}><option value="app">{classificationCopy.appScope}</option><option value="app-title">{classificationCopy.titleScope}</option></select></label>
                  {ruleDraft.scope === "app-title" && <label>{classificationCopy.pattern}<input value={ruleDraft.titlePattern ?? ""} maxLength={256} onChange={(event) => setRuleDraft({ ...ruleDraft, titlePattern: event.target.value })} onBlur={() => void refreshImpact()} /></label>}
                </div>
                {ruleDraft.scope === "app" && /chrome|safari|firefox|edge|arc|brave|orion/i.test(block.app) && <p className="review-warning"><AlertCircle size={15} aria-hidden="true" />{messages(locale).review.fallbackWarning}</p>}
                {impact && <p className="rule-impact">{classificationCopy.impact(impact.sessions, formatDuration(impact.durationSeconds, locale))}</p>}
                {error && <p className="form-error" role="alert">{error}</p>}
                <div className="button-row"><button type="button" className="primary-button" disabled={saving || (ruleDraft.scope === "app-title" && !ruleDraft.titlePattern?.trim())} onClick={() => void createRule()}><Check size={16} />{classificationCopy.createRule}</button><button type="button" className="text-button" onClick={() => setEditing(undefined)}>{classificationCopy.skipRule}</button></div>
              </>}
            </form>
          )}
        </article>
      ))}
    </div>
  );
}

function reviewRange(value: "1" | "7" | "30" | "all", retentionDays: SettingsV1["retentionDays"]) {
  const to = new Date();
  const from = new Date(to);
  if (value === "1") from.setHours(0, 0, 0, 0);
  else from.setDate(from.getDate() - (value === "all" ? (retentionDays ?? 366) : Number(value)));
  return { from: from.toISOString(), to: to.toISOString() };
}

function ReviewGroupCard({ group, locale, range, onApplied }: {
  group: ReviewGroupV1;
  locale: Locale;
  range: { from: string; to: string };
  onApplied: (operation: ReviewOperationV1) => void;
}) {
  const copy = messages(locale).review;
  const [category, setCategory] = useState<CategoryId>("admin");
  const [activityLabel, setActivityLabel] = useState("");
  const [mode, setMode] = useState<"sessions" | "rule">("sessions");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const categories = mode === "rule" ? RULE_CATEGORIES : SESSION_CATEGORIES;

  async function apply() {
    if (mode === "rule" && group.browserFallback && !window.confirm(copy.fallbackWarning)) return;
    setBusy(true); setError(undefined);
    try {
      const operation = await bridge.applyReviewGroup({
        ...range, app: group.app, titlePattern: group.titlePattern, category,
        activityLabel: activityLabel.trim() || undefined, mode,
      });
      onApplied(operation);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : messages(locale).common.unexpectedError);
    } finally { setBusy(false); }
  }

  return (
    <article className="review-card">
      <div className="review-card-heading">
        <div><h2>{group.app}</h2><p>{group.titlePattern || group.sampleTitle || copy.noTitle}</p></div>
        <strong>{formatDuration(group.durationSeconds, locale)}</strong>
      </div>
      <div className="review-meta"><span>{copy.sessions(group.sessions)}</span><span>{copy.period(formatTime(group.firstSeen, locale), formatTime(group.lastSeen, locale))}</span></div>
      {group.browserFallback && <p className="review-warning"><AlertCircle size={15} aria-hidden="true" />{copy.fallbackWarning.replace(" ¿Quieres continuar?", "").replace(" Continue?", "")}</p>}
      <div className="review-fields">
        <label>{copy.category}<select value={category} onChange={(event) => setCategory(event.target.value as CategoryId)}>{categories.map((item) => <option key={item} value={item}>{categoryLabel(locale, item)}</option>)}</select></label>
        <label>{copy.activity}<input value={activityLabel} maxLength={200} placeholder={copy.activityPlaceholder} onChange={(event) => setActivityLabel(event.target.value)} /></label>
        <label>{copy.apply}<select value={mode} onChange={(event) => { const next = event.target.value as "sessions" | "rule"; setMode(next); if (next === "rule" && category === "break") setCategory("admin"); }}><option value="sessions">{copy.sessionsOnly}</option><option value="rule">{copy.createRule}</option></select></label>
      </div>
      {error && <p className="form-error" role="alert">{error}</p>}
      <div className="button-row"><button type="button" className="primary-button" disabled={busy} onClick={() => void apply()}><Check size={15} />{copy.apply}</button></div>
    </article>
  );
}

function ReviewView({ settings, onChanged }: { settings: SettingsV1; onChanged: () => Promise<void> }) {
  const copy = messages(settings.locale).review;
  const [period, setPeriod] = useState<"1" | "7" | "30" | "all">("7");
  const [groups, setGroups] = useState<ReviewGroupV1[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string>();
  const [lastOperation, setLastOperation] = useState<ReviewOperationV1>();
  const range = useMemo(() => reviewRange(period, settings.retentionDays), [period, settings.retentionDays]);

  async function refresh() {
    setLoading(true); setError(undefined);
    try { setGroups(await bridge.listReviewGroups(range.from, range.to)); }
    catch (caught) { setError(caught instanceof Error ? caught.message : messages(settings.locale).common.unexpectedError); }
    finally { setLoading(false); }
  }

  useEffect(() => { void refresh(); }, [range.from, range.to]);

  async function applied(operation: ReviewOperationV1) {
    setLastOperation(operation);
    await Promise.all([refresh(), onChanged()]);
  }

  async function undo() {
    if (!lastOperation) return;
    await bridge.undoReviewOperation({ blockIds: lastOperation.blockIds, ruleId: lastOperation.ruleId });
    setLastOperation(undefined);
    await Promise.all([refresh(), onChanged()]);
  }

  return (
    <>
      <header className="page-header"><div><p className="page-context">{copy.context}</p><h1>{copy.title}</h1><p className="page-intro">{copy.intro}</p></div><label className="review-range">{copy.range}<select value={period} onChange={(event) => setPeriod(event.target.value as typeof period)}>{Object.entries(copy.ranges).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label></header>
      {lastOperation && <div className="review-success" role="status"><span>{copy.applied(lastOperation.affectedSessions)}</span><button type="button" className="text-button" onClick={() => void undo()}><RefreshCw size={14} />{copy.undo}</button></div>}
      {loading ? <p className="inline-empty">{copy.loading}</p> : error ? <p className="form-error" role="alert">{error}</p> : groups.length === 0 ? <p className="inline-empty">{copy.empty}</p> : <div className="review-grid">{groups.map((group) => <ReviewGroupCard key={group.id} group={group} locale={settings.locale} range={range} onApplied={(operation) => void applied(operation)} />)}</div>}
    </>
  );
}

function TodayView({ settings, status, blocks, checkIns, onNavigate, onPause, onResume, onClassify, onClear, onSaveRule, onPreviewRule }: {
  settings: SettingsV1;
  status: TrackingStatus;
  blocks: TimelineBlockV1[];
  checkIns: CheckInV1[];
  onNavigate: (view: View) => void;
  onPause: () => Promise<void>;
  onResume: () => Promise<void>;
  onClassify: (input: SessionClassificationInputV1) => Promise<SessionClassificationResultV1>;
  onClear: (blockId: string) => Promise<void>;
  onSaveRule: (rule: ClassificationRuleV1) => Promise<void>;
  onPreviewRule: (rule: ClassificationRuleV1) => Promise<RuleImpactV1>;
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
          <Timeline blocks={blocks} locale={locale} onClassify={onClassify} onClear={onClear} onSaveRule={onSaveRule} onPreviewRule={onPreviewRule} />
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

function AiEvaluationSection({ days, locale, enabled }: { days: 7 | 14 | 28; locale: Locale; enabled: boolean }) {
  const [status, setStatus] = useState<AiConfigurationStatusV1>();
  const [preview, setPreview] = useState<EvaluationPayloadPreviewV1>();
  const [reports, setReports] = useState<EvaluationReportV1[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ error?: boolean; text: string }>();
  const latest = reports[0];
  const text = locale === "en" ? {
    eyebrow: "Independent AI review", title: "Analyst + evaluator", intro: "The analyst proposes opportunities and a second model rejects anything unsupported by your evidence.",
    configure: "Configure OpenRouter, two different models, and a budget in Settings.", preview: "Preview payload", authorize: "Authorize this policy", run: "Run evaluation", sources: "Sources", cost: "maximum estimated cost", rejected: "rejected by the evaluator", noApproved: "The evaluator did not approve any suggestion yet.", useful: "Useful", notUseful: "Not useful", context: "Needs context", feedbackSaved: "Feedback saved locally.",
  } : {
    eyebrow: "Revisión independiente con IA", title: "Analista + evaluador", intro: "El analista propone oportunidades y un segundo modelo rechaza todo lo que no esté sustentado por tu evidencia.",
    configure: "Configura OpenRouter, dos modelos distintos y un presupuesto en Configuración.", preview: "Revisar payload", authorize: "Autorizar esta política", run: "Ejecutar evaluación", sources: "Fuentes", cost: "costo máximo estimado", rejected: "rechazadas por el evaluador", noApproved: "El evaluador todavía no aprobó ninguna sugerencia.", useful: "Útil", notUseful: "No útil", context: "Falta contexto", feedbackSaved: "Feedback guardado localmente.",
  };

  async function refresh() {
    const [nextStatus, nextReports] = await Promise.all([bridge.getAiConfigurationStatus(), bridge.listAiReports()]);
    setStatus(nextStatus); setReports(nextReports);
  }
  useEffect(() => { void refresh().catch((error) => setMessage({ error: true, text: error instanceof Error ? error.message : String(error) })); }, [enabled]);

  async function guarded(run: () => Promise<void>) {
    setBusy(true); setMessage(undefined);
    try { await run(); } catch (error) { setMessage({ error: true, text: error instanceof Error ? error.message : String(error) }); } finally { setBusy(false); }
  }
  const feedback = (opportunityId: string, rating: "useful" | "not-useful" | "needs-context") => guarded(async () => {
    if (!latest) return;
    await bridge.saveAiFeedback({ runId: latest.id, opportunityId, rating });
    setMessage({ text: text.feedbackSaved });
  });

  return (
    <section className="ai-evaluation-section" aria-labelledby="ai-evaluation-title">
      <div className="prompt-heading"><div><p className="section-note">{text.eyebrow}</p><h2 id="ai-evaluation-title">{text.title}</h2><p>{text.intro}</p></div><Bot size={28} aria-hidden="true" /></div>
      {!enabled ? <p className="inline-empty">{text.configure}</p> : <>
        <div className="ai-evaluation-actions">
          <button type="button" className="secondary-button" disabled={busy} onClick={() => void guarded(async () => { setPreview(await bridge.previewAiEvaluation(days)); await refresh(); })}><Clipboard size={16} />{text.preview}</button>
          <button type="button" className="primary-button" disabled={busy || !status?.ready} onClick={() => void guarded(async () => { const report = await bridge.runAiEvaluation(days); setReports([report, ...(await bridge.listAiReports()).filter((item) => item.id !== report.id)]); await refresh(); })}><Sparkles size={16} />{text.run}</button>
        </div>
        {preview && <div className="ai-payload-preview">
          <p><strong>{text.sources}:</strong> {preview.sources.join(", ")} · {preview.estimatedInputTokens.toLocaleString(locale)} tokens · ${preview.estimatedMaxCostUsd.toFixed(4)} {text.cost}</p>
          <details><summary>JSON</summary><pre>{JSON.stringify(preview.payload, null, 2)}</pre></details>
          {!preview.consentCurrent && <button type="button" className="primary-button" disabled={busy} onClick={() => void guarded(async () => { await bridge.grantCloudConsent(preview.policyFingerprint); setPreview({ ...preview, consentCurrent: true }); await refresh(); })}><LockKeyhole size={15} />{text.authorize}</button>}
        </div>}
        {latest && <div className="ai-report"><div className="ai-report-meta"><time dateTime={latest.generatedAt}>{new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(new Date(latest.generatedAt))}</time><span>${latest.costUsd.toFixed(4)}</span><span>{latest.rejectedCount} {text.rejected}</span></div><p>{latest.summary}</p>
          {latest.opportunities.length === 0 ? <p className="inline-empty">{text.noApproved}</p> : latest.opportunities.map((opportunity) => <article className="ai-opportunity" key={opportunity.id}><div><span className="source-badge">{opportunity.solutionLevel}</span><h3>{opportunity.title}</h3><p>{opportunity.problem}</p></div><dl><div><dt>Evidence</dt><dd>{opportunity.evidenceIds.join(", ")}</dd></div><div><dt>7 días</dt><dd>{opportunity.experiment}</dd></div><div><dt>Métrica</dt><dd>{opportunity.successMetric}</dd></div></dl><div className="score-line"><span><b>{opportunity.evaluator.faithfulness}/5</b> evidencia</span><span><b>{opportunity.evaluator.feasibility}/5</b> factibilidad</span><span><b>{opportunity.evaluator.safety}/5</b> seguridad</span></div><div className="button-row"><button type="button" className="text-button" disabled={busy} onClick={() => void feedback(opportunity.id, "useful")}>{text.useful}</button><button type="button" className="text-button" disabled={busy} onClick={() => void feedback(opportunity.id, "not-useful")}>{text.notUseful}</button><button type="button" className="text-button" disabled={busy} onClick={() => void feedback(opportunity.id, "needs-context")}>{text.context}</button></div></article>)}
        </div>}
      </>}
      {message && <p className={message.error ? "form-error" : "form-success"} role={message.error ? "alert" : "status"}>{message.text}</p>}
    </section>
  );
}

function AutomateView({ diagnostic, locale, busy, aiAvailable, aiEnabled, days, onGenerate, onCopy, onOpen, onMapWorkflow, onOpenBlueprint, blueprintCandidate }: { diagnostic: DiagnosticV1; locale: Locale; busy: boolean; aiAvailable: boolean; aiEnabled: boolean; days: 7 | 14 | 28; onGenerate: () => Promise<void>; onCopy: () => Promise<void>; onOpen: (url: string) => Promise<void>; onMapWorkflow: (candidate: AutomationCandidateV1) => void; onOpenBlueprint: (candidate: AutomationCandidateV1) => void; blueprintCandidate: AutomationCandidateV1 | null }) {
  const copy = messages(locale).automate;
  const candidate = diagnostic.candidates[0];
  return (
    <>
      <header className="page-header"><div><p className="page-context">{copy.candidate}</p><h1>{copy.title}</h1><p className="page-intro">{copy.intro}</p></div><button type="button" className="secondary-button" disabled={busy} onClick={() => void onGenerate()}><RefreshCw size={16} aria-hidden="true" />{copy.refresh}</button></header>
      {diagnostic.preliminary && <div className="preliminary-banner"><Gauge size={18} aria-hidden="true" /><span><strong>{copy.preliminaryTitle}</strong>{copy.preliminaryCopy(diagnostic.daysObserved.toFixed(1))}</span></div>}
      {candidate ? (
        <section className="candidate-section" aria-labelledby="candidate-title">
          <div className="candidate-rank"><span>{candidate.priorityScore}</span><small>/ 100</small></div>
          <div className="candidate-main"><p className="section-note">{copy.candidate}</p><h2 id="candidate-title">{candidate.activityLabel}</h2><p>{candidate.evidence.join(" · ")}</p><div className="score-line"><span><b>{candidate.priorityScore}</b>/100 {copy.signal}</span><span><b>{candidate.estimatedWeeklyHours}</b> {copy.weeklyHours}</span><span><b>{candidate.weeklyFrequency}</b> {copy.repetitions}</span></div><div className="workflow-cta"><button type="button" className="secondary-button" onClick={() => onMapWorkflow(candidate)}>{copy.workflowAction}<ChevronRight size={16} aria-hidden="true" /></button><small>{copy.workflowHint}</small></div></div>
          <div className="candidate-scores"><span><b>{candidate.impactScore}/40</b>{copy.impact}</span><span><b>{candidate.feasibilityScore}/35</b>{copy.feasibility}</span><span><b>{candidate.riskScore}/25</b>{copy.risk}</span></div>
        </section>
      ) : <section className="empty-state"><Target size={30} aria-hidden="true" /><h2>{copy.empty}</h2><p>{copy.emptyCopy}</p></section>}
      <section className="recommendations"><h2>{copy.recommendations}</h2><ol>{diagnostic.recommendations.map((recommendation, index) => <li key={recommendation}><span>{String(index + 1).padStart(2, "0")}</span><p>{recommendation}</p></li>)}</ol></section>
      {aiAvailable && <AiEvaluationSection days={days} locale={locale} enabled={aiEnabled} />}
      <section className="prompt-section" aria-labelledby="prompt-title">
        <div className="prompt-heading"><div><p className="section-note">{copy.promptIntro}</p><h2 id="prompt-title">{copy.promptTitle}</h2></div><button type="button" className="primary-button" onClick={() => void onCopy()}><Clipboard size={16} aria-hidden="true" />{copy.copyPrompt}</button></div>
        <pre tabIndex={0}>{diagnostic.prompt}</pre>
        <div className="prompt-links"><button type="button" onClick={() => void onOpen("https://chatgpt.com/")}>{copy.openChatGpt} <ArrowUpRight size={15} aria-hidden="true" /></button><button type="button" onClick={() => void onOpen("https://claude.ai/new")}>{copy.openClaude} <ArrowUpRight size={15} aria-hidden="true" /></button><span><LockKeyhole size={14} aria-hidden="true" />{copy.outboundNotice}</span></div>
      </section>
      {blueprintCandidate ? <section className="blueprint-entry-card" aria-labelledby="blueprint-entry-title"><div><p className="section-note">{messages(locale).blueprint.context}</p><h2 id="blueprint-entry-title">{blueprintCandidate.activityLabel}</h2><p>{messages(locale).blueprint.intro}</p></div><button type="button" className="primary-button" data-testid="blueprint-entry" onClick={() => onOpenBlueprint(blueprintCandidate)}>{messages(locale).blueprint.openAction}<ChevronRight size={16} aria-hidden="true" /></button></section> : <section className="empty-state blueprint-no-candidate" data-testid="blueprint-no-candidate"><Target size={30} aria-hidden="true" /><h2>{copy.empty}</h2><p>{copy.emptyCopy}</p></section>}
    </>
  );
}

function ClassificationRulesSettings({ rules, locale, onSave, onDelete }: { rules: ClassificationRuleV1[]; locale: Locale; onSave: (rule: ClassificationRuleV1) => Promise<void>; onDelete: (id: string) => Promise<void> }) {
  const copy = messages(locale).classification;
  const [drafts, setDrafts] = useState(rules);
  const [busy, setBusy] = useState<string>();
  useEffect(() => setDrafts(rules), [rules]);
  const update = (id: string, change: Partial<ClassificationRuleV1>) => setDrafts((items) => items.map((item) => item.id === id ? { ...item, ...change } : item));
  return (
    <section className="settings-group classification-rules-settings">
      <div className="settings-intro"><Target size={19} aria-hidden="true" /><div><h2>{copy.rulesTitle}</h2><p>{copy.rulesHint}</p></div></div>
      <div className="settings-fields">
        {drafts.length === 0 ? <p className="inline-empty">{copy.noRules}</p> : drafts.map((rule) => (
          <div className="rule-settings-row" key={rule.id}>
            <div className="rule-settings-heading"><strong>{rule.app}</strong><small>{rule.scope === "app" ? copy.appScope : rule.scope === "app-title" ? copy.titleScope : copy.legacyScope}</small></div>
            <div className="two-columns">
              {rule.scope === "app-title" && <label>{copy.pattern}<input value={rule.titlePattern ?? ""} maxLength={256} onChange={(event) => update(rule.id, { titlePattern: event.target.value })} /></label>}
              <label>{copy.category}<select value={rule.category} onChange={(event) => update(rule.id, { category: event.target.value as CategoryId })}>{RULE_CATEGORIES.map((category) => <option key={category} value={category}>{categoryLabel(locale, category)}</option>)}</select></label>
              <label>{copy.activityLabel}<input value={rule.activityLabel ?? ""} maxLength={200} onChange={(event) => update(rule.id, { activityLabel: event.target.value || undefined })} placeholder={copy.activityPlaceholder} /></label>
            </div>
            <div className="button-row"><button type="button" className="secondary-button" disabled={busy === rule.id || (rule.scope === "app-title" && !rule.titlePattern?.trim())} onClick={() => { setBusy(rule.id); void onSave(rule).catch(() => undefined).finally(() => setBusy(undefined)); }}><Check size={15} />{copy.saveRule}</button><button type="button" className="text-button danger-text" disabled={busy === rule.id} onClick={() => { setBusy(rule.id); void onDelete(rule.id).catch(() => undefined).finally(() => setBusy(undefined)); }}><Trash2 size={15} />{copy.deleteRule}</button></div>
          </div>
        ))}
      </div>
    </section>
  );
}

function IntelligenceSettings({ draft, saved, experimental, onChange }: { draft: SettingsV1; saved: SettingsV1; experimental: boolean; onChange: (value: SettingsV1) => void }) {
  const [models, setModels] = useState<OpenRouterModelV1[]>([]);
  const [aiStatus, setAiStatus] = useState<AiConfigurationStatusV1>();
  const [clients, setClients] = useState<ConnectorClientV1[]>([]);
  const [pairing, setPairing] = useState<ConnectorPairingV1>();
  const [ocrStatus, setOcrStatus] = useState<ScreenOcrStatusV1>();
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ error?: boolean; text: string }>();
  const locale = draft.locale;
  const t = locale === "en" ? {
    title: "Deep context and AI", intro: "Every source is opt-in. Cloud use is separate from local capture.", local: "Capture locally", cloud: "Allow in approved cloud summaries", browser: "Active browser tab", editor: "VS Code / Cursor project", shell: "Terminal project", screenshots: "Local screen OCR", pair: "Pair", saveFirst: "Save this source before pairing.", interval: "OCR interval", sites: "Sites allowed for visible page text", excluded: "Apps excluded from screenshots", ai: "OpenRouter evaluation", key: "OpenRouter API key", saveKey: "Save securely", deleteKey: "Remove key", analyst: "Analyst model", evaluator: "Evaluator model", budget: "Monthly hard limit (USD)", weekly: "Run an automatic weekly evaluation", weeklyDay: "Day", weeklyHour: "Hour", enableAi: "Enable analyst + independent evaluator", privacy: "Requests require ZDR, deny data collection, strict JSON and no provider fallbacks.", connected: "Paired connectors", revoke: "Revoke", code: "Pairing code", requestScreen: "Allow screen recording", permissionReady: "Screen recording authorized", unsupportedOcr: "Native OCR is unavailable on this system", localTitle: "Local context connectors", localIntro: "Every source is opt-in and stays on this computer.",
  } : {
    title: "Contexto profundo e IA", intro: "Cada fuente es opcional. Autorizar nube es independiente de capturar localmente.", local: "Capturar localmente", cloud: "Permitir en resúmenes de nube aprobados", browser: "Pestaña activa del navegador", editor: "Proyecto de VS Code / Cursor", shell: "Proyecto de terminal", screenshots: "OCR local de pantalla", pair: "Emparejar", saveFirst: "Guarda esta fuente antes de emparejar.", interval: "Intervalo de OCR", sites: "Sitios autorizados para leer texto visible", excluded: "Apps excluidas de capturas", ai: "Evaluación con OpenRouter", key: "API key de OpenRouter", saveKey: "Guardar de forma segura", deleteKey: "Eliminar key", analyst: "Modelo analista", evaluator: "Modelo evaluador", budget: "Límite mensual estricto (USD)", weekly: "Ejecutar una evaluación semanal automática", weeklyDay: "Día", weeklyHour: "Hora", enableAi: "Activar analista + evaluador independiente", privacy: "Las solicitudes exigen ZDR, niegan recolección, usan JSON estricto y no permiten fallbacks de proveedor.", connected: "Conectores emparejados", revoke: "Revocar", code: "Código de emparejamiento", requestScreen: "Autorizar grabación de pantalla", permissionReady: "Grabación de pantalla autorizada", unsupportedOcr: "El OCR nativo no está disponible en este sistema", localTitle: "Conectores de contexto local", localIntro: "Cada fuente es opcional y se queda en esta computadora.",
  };

  async function refresh(includeModels = false) {
    if (!experimental) {
      setClients(await bridge.listConnectorClients());
      return;
    }
    const [status, nextClients, nextOcrStatus] = await Promise.all([bridge.getAiConfigurationStatus(), bridge.listConnectorClients(), bridge.getScreenOcrStatus()]);
    setAiStatus(status); setClients(nextClients); setOcrStatus(nextOcrStatus);
    if (includeModels) setModels(await bridge.listOpenRouterModels());
  }
  useEffect(() => { void refresh(true).catch((error) => setMessage({ error: true, text: error instanceof Error ? error.message : String(error) })); }, []);

  function updateSource(source: "browser" | "editor" | "shell" | "screenshots", change: Partial<SettingsV1["deepCapture"]["browser"]>) {
    onChange({ ...draft, deepCapture: { ...draft.deepCapture, [source]: { ...draft.deepCapture[source], ...change } } });
  }
  async function guarded(run: () => Promise<void>) {
    setBusy(true); setMessage(undefined);
    try { await run(); } catch (error) { setMessage({ error: true, text: error instanceof Error ? error.message : String(error) }); } finally { setBusy(false); }
  }
  const localSources = [["browser", t.browser, "Chromium"], ["editor", t.editor, "VS Code / Cursor"], ["shell", t.shell, "Terminal"]] as const;
  const sources = experimental ? [...localSources, ["screenshots", t.screenshots, "OCR"] as const] : localSources;
  const weekdays = locale === "en" ? ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"] : ["Domingo", "Lunes", "Martes", "Miércoles", "Jueves", "Viernes", "Sábado"];
  const modelOptions = (selected?: string) => <>{selected && !models.some((model) => model.id === selected) && <option value={selected}>{selected}</option>}{models.map((model) => <option key={model.id} value={model.id}>{model.name} · ${((model.promptPrice + model.completionPrice) * 1_000_000).toFixed(2)}/M token</option>)}</>;

  return <section className="settings-group intelligence-settings">
    <div className="settings-intro"><Bot size={19} aria-hidden="true" /><div><h2>{experimental ? t.title : t.localTitle}</h2><p>{experimental ? t.intro : t.localIntro}</p></div></div>
    <div className="settings-fields">
      <div className="source-policy-list">{sources.map(([source, label, name]) => {
        const policy = draft.deepCapture[source];
        const canPair = source !== "screenshots" && saved.deepCapture[source].enabled;
        const ocrDetail = ocrStatus?.permission === "granted" ? ` · ${t.permissionReady}` : ocrStatus && !ocrStatus.supported ? ` · ${t.unsupportedOcr}` : "";
        return <div className="source-policy" key={source}><div><strong>{label}</strong>{source === "screenshots" && <small>{t.interval}: {draft.deepCapture.screenshotIntervalMinutes} min · los píxeles nunca se guardan{ocrDetail}</small>}</div><label><input type="checkbox" checked={policy.enabled} disabled={source === "screenshots" && ocrStatus?.supported === false} onChange={(event) => updateSource(source, { enabled: event.target.checked })} />{t.local}</label>{experimental && <label><input type="checkbox" checked={policy.cloudAllowed} disabled={!policy.enabled} onChange={(event) => updateSource(source, { cloudAllowed: event.target.checked })} />{t.cloud}</label>}{source !== "screenshots" ? <button type="button" className="text-button" disabled={!canPair || busy} title={canPair ? undefined : t.saveFirst} onClick={() => void guarded(async () => { setPairing(await bridge.createConnectorPairing(source as "browser" | "editor" | "shell", name)); })}>{t.pair}</button> : ocrStatus?.supported && ocrStatus.permission !== "granted" ? <button type="button" className="text-button" disabled={busy} onClick={() => void guarded(async () => { setOcrStatus(await bridge.requestScreenOcrPermission()); })}>{t.requestScreen}</button> : <span />}</div>;
      })}</div>
      <div className="two-columns">{experimental && <label>{t.interval}<select value={draft.deepCapture.screenshotIntervalMinutes} onChange={(event) => onChange({ ...draft, deepCapture: { ...draft.deepCapture, screenshotIntervalMinutes: Number(event.target.value) as 5 | 15 | 30 | 60 } })}>{[5, 15, 30, 60].map((minutes) => <option key={minutes} value={minutes}>{minutes} min</option>)}</select></label>}<label>{t.sites}<textarea rows={3} value={draft.deepCapture.browserTextAllowlist.join("\n")} onChange={(event) => onChange({ ...draft, deepCapture: { ...draft.deepCapture, browserTextAllowlist: event.target.value.split(/\s+/).filter(Boolean) } })} placeholder="notion.so&#10;docs.google.com" /></label></div>
      {experimental && <label>{t.excluded}<textarea rows={2} value={draft.deepCapture.excludedApps.join("\n")} onChange={(event) => onChange({ ...draft, deepCapture: { ...draft.deepCapture, excludedApps: event.target.value.split("\n").map((value) => value.trim()).filter(Boolean) } })} /></label>}
      {pairing && <div className="pairing-code"><span>{t.code}</span><strong>{pairing.code}</strong><small>127.0.0.1:{pairing.serverPort ?? "—"} · {new Intl.DateTimeFormat(locale, { timeStyle: "short" }).format(new Date(pairing.expiresAt))}</small></div>}
      {clients.length > 0 && <div><p className="field-group-title">{t.connected}</p>{clients.filter((client) => !client.revokedAt).map((client) => <div className="connector-row" key={client.id}><span><strong>{client.name}</strong><small>{client.kind} · {client.lastSeenAt ? formatTime(client.lastSeenAt, locale) : "—"}</small></span><button type="button" className="text-button danger-text" onClick={() => void guarded(async () => { await bridge.revokeConnectorClient(client.id); await refresh(); })}>{t.revoke}</button></div>)}</div>}
      {experimental && <>
      <hr />
      <p className="field-group-title">{t.ai}</p><p className="field-group-help">{t.privacy}</p>
      <div className="api-key-row"><label>{t.key}<input type="password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder={aiStatus?.hasApiKey ? "••••••••••••" : "sk-or-v1-…"} autoComplete="off" /></label><button type="button" className="secondary-button" disabled={busy || !apiKey.trim()} onClick={() => void guarded(async () => { await bridge.saveOpenRouterApiKey(apiKey); setApiKey(""); await refresh(); })}>{t.saveKey}</button>{aiStatus?.hasApiKey && <button type="button" className="text-button danger-text" onClick={() => void guarded(async () => { await bridge.deleteOpenRouterApiKey(); await refresh(); })}>{t.deleteKey}</button>}</div>
      <div className="two-columns"><label>{t.analyst}<select value={draft.ai.analystModel ?? ""} onChange={(event) => onChange({ ...draft, ai: { ...draft.ai, analystModel: event.target.value || undefined } })}><option value="">—</option>{modelOptions(draft.ai.analystModel)}</select></label><label>{t.evaluator}<select value={draft.ai.evaluatorModel ?? ""} onChange={(event) => onChange({ ...draft, ai: { ...draft.ai, evaluatorModel: event.target.value || undefined } })}><option value="">—</option>{modelOptions(draft.ai.evaluatorModel)}</select></label></div>
      <label>{t.budget}<input type="number" min="0.01" max="10000" step="0.01" value={draft.ai.monthlyBudgetUsd ?? ""} onChange={(event) => onChange({ ...draft, ai: { ...draft.ai, monthlyBudgetUsd: event.target.value ? Number(event.target.value) : undefined } })} /></label>
      <label className="toggle-row"><span><strong>{t.enableAi}</strong><small>{aiStatus ? `$${aiStatus.spentThisMonthUsd.toFixed(4)} usado este mes` : ""}</small></span><input type="checkbox" role="switch" checked={draft.ai.enabled} onChange={(event) => onChange({ ...draft, ai: { ...draft.ai, enabled: event.target.checked } })} /></label>
      <label className="toggle-row"><span><strong>{t.weekly}</strong></span><input type="checkbox" role="switch" checked={draft.ai.weeklyEnabled} disabled={!draft.ai.enabled} onChange={(event) => onChange({ ...draft, ai: { ...draft.ai, weeklyEnabled: event.target.checked } })} /></label>
      {draft.ai.weeklyEnabled && <div className="two-columns"><label>{t.weeklyDay}<select value={draft.ai.weeklyDay} onChange={(event) => onChange({ ...draft, ai: { ...draft.ai, weeklyDay: Number(event.target.value) } })}>{weekdays.map((day, index) => <option key={day} value={index}>{day}</option>)}</select></label><label>{t.weeklyHour}<select value={draft.ai.weeklyHour} onChange={(event) => onChange({ ...draft, ai: { ...draft.ai, weeklyHour: Number(event.target.value) } })}>{Array.from({ length: 24 }, (_, hour) => <option key={hour} value={hour}>{String(hour).padStart(2, "0")}:00</option>)}</select></label></div>}
      </>}
      {message && <p className={message.error ? "form-error" : "form-success"} role={message.error ? "alert" : "status"}>{message.text}</p>}
    </div>
  </section>;
}

function SettingsView({ settings, status, classificationRules, onSave, onSaveRule, onDeleteRule, onExport, onDelete }: { settings: SettingsV1; status: TrackingStatus; classificationRules: ClassificationRuleV1[]; onSave: (value: SettingsV1) => Promise<void>; onSaveRule: (rule: ClassificationRuleV1) => Promise<void>; onDeleteRule: (id: string) => Promise<void>; onExport: (format: "json" | "csv") => Promise<void>; onDelete: () => Promise<void> }) {
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
      <IntelligenceSettings draft={draft} saved={settings} experimental={status.experimentalIntelligence} onChange={setDraft} />
      <ClassificationRulesSettings rules={classificationRules} locale={draft.locale} onSave={onSaveRule} onDelete={onDeleteRule} />
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
  const [classificationRules, setClassificationRules] = useState<ClassificationRuleV1[]>([]);

  async function load() {
    setLoading(true);
    setLoadError(undefined);
    try {
      const from = new Date(); from.setHours(0, 0, 0, 0);
      const to = new Date(); to.setHours(23, 59, 59, 999);
      const [nextSettings, nextStatus, nextBlocks, nextCheckIns, nextStudy, nextWorkflowIntakes, nextClassificationRules] = await Promise.all([
        bridge.getSettings(), bridge.getTrackingStatus(), bridge.getTimeline(from.toISOString(), to.toISOString()), bridge.listCheckIns(), bridge.getStudyProgress(), bridge.listWorkflowIntakes(), bridge.listClassificationRules(),
      ]);
      setSettings(nextSettings); setStatus(nextStatus); setBlocks(nextBlocks); setCheckIns(nextCheckIns); setStudy(nextStudy); setWorkflowIntakes(nextWorkflowIntakes); setClassificationRules(nextClassificationRules);
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

  async function fetchTodaySnapshot() {
    const from = new Date(); from.setHours(0, 0, 0, 0);
    const to = new Date(); to.setHours(23, 59, 59, 999);
    return Promise.all([
      bridge.getTrackingStatus(),
      bridge.getTimeline(from.toISOString(), to.toISOString()),
    ]);
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
    if (!settings?.onboardingCompleted || view !== "today") return;
    let disposed = false;
    let refreshing = false;
    let reportedError = false;

    const refreshToday = async () => {
      if (document.visibilityState !== "visible" || refreshing) return;
      refreshing = true;
      try {
        const [nextStatus, nextBlocks] = await fetchTodaySnapshot();
        if (!disposed) {
          setStatus(nextStatus);
          setBlocks(nextBlocks);
          reportedError = false;
        }
      } catch (error) {
        if (!disposed && !reportedError) {
          setNotice({
            kind: "error",
            message: error instanceof Error
              ? error.message
              : messages(settings.locale).common.unexpectedError,
          });
          reportedError = true;
        }
      } finally {
        refreshing = false;
      }
    };

    const onVisibilityChange = () => {
      if (document.visibilityState === "visible") void refreshToday();
    };
    const timer = window.setInterval(() => void refreshToday(), 60_000);
    document.addEventListener("visibilitychange", onVisibilityChange);
    void refreshToday();
    return () => {
      disposed = true;
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [settings?.onboardingCompleted, settings?.locale, view]);
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
    { id: "review", label: copy.nav.review, icon: ListChecks },
    { id: "checkins", label: copy.nav.checkins, icon: HelpCircle },
    { id: "study", label: copy.nav.study, icon: Focus },
    { id: "automate", label: copy.nav.automate, icon: Sparkles },
    { id: "settings", label: copy.nav.settings, icon: SettingsIcon },
  ];
  const saveSettings = async (value: SettingsV1) => action(async () => { setSettings(await bridge.saveSettings(value)); }, messages(value.locale).notices.settingsSaved);
  const answer = async (value: CheckInAnswer) => action(async () => { await bridge.answerCheckIn(value); setCheckIns(await bridge.listCheckIns()); setStatus(await bridge.getTrackingStatus()); }, copy.notices.contextSaved);
  const classifySession = async (input: SessionClassificationInputV1) => {
    const result = await bridge.saveSessionClassification(input);
    setBlocks((current) => current.map((block) => block.id === result.block.id ? result.block : block));
    setNotice({ kind: "success", message: copy.notices.contextSaved });
    return result;
  };
  const clearSessionClassification = async (blockId: string) => {
    const block = await bridge.clearSessionClassification(blockId);
    setBlocks((current) => current.map((item) => item.id === block.id ? block : item));
  };
  const saveClassificationRule = async (rule: ClassificationRuleV1) => {
    try {
      await bridge.saveClassificationRule(rule);
      const [nextRules, snapshot] = await Promise.all([bridge.listClassificationRules(), fetchTodaySnapshot()]);
      setClassificationRules(nextRules); setStatus(snapshot[0]); setBlocks(snapshot[1]);
      setNotice({ kind: "success", message: copy.notices.settingsSaved });
    } catch (error) {
      setNotice({ kind: "error", message: error instanceof Error ? error.message : copy.common.unexpectedError });
      throw error;
    }
  };
  const deleteClassificationRule = async (id: string) => {
    try {
      await bridge.deleteClassificationRule(id);
      const [nextRules, snapshot] = await Promise.all([bridge.listClassificationRules(), fetchTodaySnapshot()]);
      setClassificationRules(nextRules); setStatus(snapshot[0]); setBlocks(snapshot[1]);
    } catch (error) {
      setNotice({ kind: "error", message: error instanceof Error ? error.message : copy.common.unexpectedError });
      throw error;
    }
  };
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
        <nav aria-label="Principal">{items.map(({ id, label, icon: Icon }) => <button type="button" key={id} className={view === id ? "active" : ""} aria-current={view === id ? "page" : undefined} aria-label={label} title={label} onClick={() => setView(id)}><Icon size={18} aria-hidden="true" /><span>{label}</span>{id === "checkins" && status.pendingCheckIns > 0 && <b aria-label={`${status.pendingCheckIns}`}>{status.pendingCheckIns}</b>}{id === "review" && blocks.some((block) => block.category === "unknown") && <b aria-label={`${blocks.filter((block) => block.category === "unknown").length}`}>{blocks.filter((block) => block.category === "unknown").length}</b>}</button>)}</nav>
        <div className="sidebar-footer"><StatusPill status={status} locale={settings.locale} /><p><LockKeyhole size={13} aria-hidden="true" />{status.dataStaysLocal ? copy.common.local : (settings.locale === "en" ? "Cloud only with consent" : "Nube solo con consentimiento")}</p><small>Ensō Time v0.2 beta</small></div>
      </aside>
      <main className="content" id="main-content" tabIndex={-1}>
        {status.captureIssue && <CaptureIssueBanner issue={status.captureIssue} locale={settings.locale} onRetry={() => action(async () => { setStatus(await bridge.retryCapture()); })} onRequestAccessibility={() => action(async () => { await bridge.requestAccessibilityPermission(); })} />}
        {view === "today" && <TodayView settings={settings} status={status} blocks={blocks} checkIns={checkIns} onNavigate={setView} onPause={() => action(async () => { setStatus(await bridge.pauseTracking(30)); }, copy.notices.paused)} onResume={() => action(async () => { setStatus(await bridge.resumeTracking()); }, copy.notices.resumed)} onClassify={classifySession} onClear={clearSessionClassification} onSaveRule={saveClassificationRule} onPreviewRule={bridge.previewClassificationRule} />}
        {view === "review" && <ReviewView settings={settings} onChanged={async () => { const [nextRules, snapshot] = await Promise.all([bridge.listClassificationRules(), fetchTodaySnapshot()]); setClassificationRules(nextRules); setStatus(snapshot[0]); setBlocks(snapshot[1]); }} />}
        {view === "checkins" && <CheckInsView checkIns={checkIns} settings={settings} onAnswer={answer} />}
        {view === "study" && <StudyView progress={study} locale={settings.locale} onDays={selectDays} />}
        {view === "automate" && <AutomateView diagnostic={diagnostic} locale={settings.locale} busy={diagnosticBusy} aiAvailable={status.experimentalIntelligence} aiEnabled={settings.ai.enabled} days={study.selectedDays} onGenerate={async () => { setDiagnosticBusy(true); await action(async () => { setDiagnostic(await bridge.generateDiagnostic(study.selectedDays)); }, copy.notices.diagnosticUpdated); setDiagnosticBusy(false); }} onCopy={() => action(async () => { await navigator.clipboard.writeText(diagnostic.prompt); }, copy.notices.promptCopied)} onOpen={(url) => action(() => bridge.openExternal(url))} onMapWorkflow={(candidate) => { setWorkflowCandidate(candidate); setView("workflow"); }} onOpenBlueprint={(candidate) => { setWorkflowCandidate(candidate); setView("blueprint"); }} blueprintCandidate={blueprintEntry} />}
        {view === "workflow" && <WorkflowIntakeView candidate={workflowCandidate} locale={settings.locale} onBack={() => setView("automate")} onSaved={saveWorkflowIntake} />}
        {view === "blueprint" && <AutomationBlueprintView candidate={workflowCandidate} locale={settings.locale} onBack={() => setView("automate")} onMapWorkflow={(candidate) => { setWorkflowCandidate(candidate); setView("workflow"); }} onDeleted={() => { setView("automate"); setNotice({ kind: "success", message: copy.blueprint.deleted }); }} />}
        {view === "settings" && <SettingsView settings={settings} status={status} classificationRules={classificationRules} onSave={saveSettings} onSaveRule={saveClassificationRule} onDeleteRule={deleteClassificationRule} onExport={(format) => action(async () => { const path = await bridge.exportData(format); setNotice({ kind: "success", message: copy.notices.exported(path) }); })} onDelete={() => action(async () => { if (!window.confirm(copy.settings.deleteConfirm)) return; await bridge.deleteAllData(); setNotice({ kind: "success", message: copy.notices.deleted }); await load(); })} />}
      </main>
      {notice && <div className={`toast ${notice.kind === "error" ? "is-error" : ""}`} role={notice.kind === "error" ? "alert" : "status"}>{notice.kind === "error" ? <AlertCircle size={15} aria-hidden="true" /> : <Check size={15} aria-hidden="true" />}{notice.message}</div>}
    </div>
  );
}
