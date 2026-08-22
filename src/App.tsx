import { useEffect, useMemo, useState } from "react";
import {
  Activity,
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
  RotateCcw,
  Settings as SettingsIcon,
  Sparkles,
  Target,
  Trash2,
} from "lucide-react";
import { bridge } from "./lib/bridge";
import { CATEGORY_ORDER, categoryColor } from "./lib/categories";
import { categoryLabel, formatDuration, formatTime, messages } from "./lib/i18n";
import type {
  CategoryId,
  CheckInAnswer,
  CheckInV1,
  DiagnosticV1,
  SettingsV1,
  StudyProgress,
  TimelineBlockV1,
  TrackingStatus,
} from "./lib/types";

type View = "today" | "checkins" | "study" | "automate" | "settings";

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

function StatusPill({ status }: { status: TrackingStatus }) {
  return (
    <span className={`status-pill ${status.running ? "is-running" : "is-paused"}`}>
      <span className="status-dot" />
      {status.running ? "Registrando" : "En pausa"}
    </span>
  );
}

function Onboarding({ initial, onComplete }: { initial: SettingsV1; onComplete: (value: SettingsV1) => Promise<void> }) {
  const [step, setStep] = useState(0);
  const [role, setRole] = useState(initial.role);
  const [goal, setGoal] = useState(initial.goals[0] ?? "");
  const steps = [
    {
      eyebrow: "Tu tiempo, en privado",
      title: "Entiende tu día sin llenar cronómetros.",
      copy: "Ensō Time observa aplicaciones activas en este computador y solo pregunta cuando falta contexto. Los datos no salen de tu equipo.",
      icon: LockKeyhole,
    },
    {
      eyebrow: "Una pregunta breve",
      title: "Completa únicamente los intervalos ambiguos.",
      copy: "Cada 15 minutos revisamos si la actividad quedó clara. Si lo está, no interrumpimos. Si no, puedes responder después.",
      icon: HelpCircle,
    },
    {
      eyebrow: "Automatiza con evidencia",
      title: "Convierte patrones repetidos en una decisión.",
      copy: "Después de 7, 14 o 28 días recibirás un diagnóstico y un prompt listo para analizar tu primera automatización.",
      icon: Bot,
    },
  ];
  const current = steps[step] ?? steps[0]!;
  const Icon = current.icon;

  async function finish() {
    await onComplete({
      ...initial,
      onboardingCompleted: true,
      role: role.trim(),
      goals: goal.trim() ? [goal.trim()] : initial.goals,
    });
  }

  return (
    <main className="onboarding-shell">
      <div className="onboarding-top"><BrandMark /></div>
      <section className="onboarding-panel">
        <div className="onboarding-visual"><Icon size={38} strokeWidth={1.5} /></div>
        <p className="eyebrow">{current.eyebrow}</p>
        <h1>{current.title}</h1>
        <p className="lede">{current.copy}</p>
        {step === 2 && (
          <div className="onboarding-fields">
            <label>¿Cuál es tu rol?<input value={role} onChange={(event) => setRole(event.target.value)} placeholder="Ej. Operador de negocio digital" /></label>
            <label>¿Qué tiempo quieres recuperar?<input value={goal} onChange={(event) => setGoal(event.target.value)} placeholder="Ej. Reducir reportes manuales" /></label>
          </div>
        )}
        <div className="onboarding-actions">
          <div className="step-dots" aria-label={`Paso ${step + 1} de 3`}>
            {steps.map((_, index) => <span key={index} className={index === step ? "active" : ""} />)}
          </div>
          <button className="primary-button" onClick={() => step < 2 ? setStep(step + 1) : void finish()}>
            {step < 2 ? "Continuar" : "Comenzar estudio"}<ChevronRight size={17} />
          </button>
        </div>
      </section>
    </main>
  );
}

function Timeline({ blocks, locale }: { blocks: TimelineBlockV1[]; locale: SettingsV1["locale"] }) {
  return (
    <div className="timeline" aria-label="Línea de tiempo de hoy">
      {blocks.map((block) => (
        <article className="timeline-row" key={block.id}>
          <time>{formatTime(block.start, locale)}</time>
          <span className="timeline-rail" style={{ "--category-color": categoryColor(block.category) } as React.CSSProperties} />
          <div className="timeline-copy">
            <div><strong>{block.app}</strong><span>{formatDuration(block.durationSeconds, locale)}</span></div>
            <p>{block.title || categoryLabel(locale, block.category)}</p>
          </div>
          <span className="category-tag"><i style={{ background: categoryColor(block.category) }} />{categoryLabel(locale, block.category)}</span>
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
        <div><p className="eyebrow">{date}</p><h1>Así se ve tu día.</h1><p className="page-intro">{formatDuration(totalSeconds, locale)} observados · la información sigue en este equipo.</p></div>
        <button className="secondary-button" onClick={() => void (status.running ? onPause() : onResume())}>
          {status.running ? <Pause size={16} /> : <Play size={16} />}{status.running ? "Pausar 30 min" : "Reanudar"}
        </button>
      </header>

      {pending && (
        <button className="context-callout" onClick={() => onNavigate("checkins")}>
          <span className="callout-icon"><HelpCircle size={19} /></span>
          <span><strong>Falta contexto en un intervalo</strong><small>{formatTime(pending.intervalStart, locale)}–{formatTime(pending.intervalEnd, locale)} · responder toma menos de 20 segundos</small></span>
          <ChevronRight size={18} />
        </button>
      )}

      <div className="today-layout">
        <section className="section-block">
          <div className="section-heading"><div><p className="eyebrow">Registro continuo</p><h2>Línea de tiempo</h2></div><StatusPill status={status} /></div>
          <Timeline blocks={blocks} locale={locale} />
        </section>
        <aside className="day-summary">
          <p className="eyebrow">Distribución</p>
          <h2>En qué se fue</h2>
          <div className="distribution-list">
            {totals.map((item) => (
              <div className="distribution-item" key={item.category}>
                <div><span><i style={{ background: categoryColor(item.category) }} />{categoryLabel(locale, item.category)}</span><b>{formatDuration(item.seconds, locale)}</b></div>
                <span className="bar"><i style={{ width: `${Math.max(4, item.seconds / totalSeconds * 100)}%`, background: categoryColor(item.category) }} /></span>
              </div>
            ))}
          </div>
          <div className="privacy-note"><LockKeyhole size={16} /><span><strong>Privado por diseño</strong>Aplicaciones y títulos de ventana permanecen locales.</span></div>
        </aside>
      </div>
    </>
  );
}

function CheckInsView({ checkIns, settings, onAnswer }: { checkIns: CheckInV1[]; settings: SettingsV1; onAnswer: (value: CheckInAnswer) => Promise<void> }) {
  const pending = checkIns.filter((item) => item.status === "pending");
  const history = checkIns.filter((item) => item.status === "answered");
  const active = pending[0];
  const [activityLabel, setActivityLabel] = useState("");
  const [category, setCategory] = useState<CategoryId>("admin");
  const [project, setProject] = useState("");
  const [repetitive, setRepetitive] = useState(false);
  const [rulesClear, setRulesClear] = useState(false);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (!active || !activityLabel.trim()) return;
    await onAnswer({ id: active.id, activityLabel: activityLabel.trim(), category, project: project.trim() || undefined, repetitive, rulesClear });
    setActivityLabel(""); setProject(""); setRepetitive(false); setRulesClear(false);
  }

  return (
    <>
      <header className="page-header"><div><p className="eyebrow">Contexto humano</p><h1>Completa los huecos.</h1><p className="page-intro">Ensō Time pregunta solo cuando las aplicaciones no cuentan toda la historia.</p></div></header>
      {active ? (
        <form className="checkin-form" onSubmit={(event) => void submit(event)}>
          <div className="interval-label"><Clock3 size={18} /><span><strong>{formatTime(active.intervalStart, settings.locale)}–{formatTime(active.intervalEnd, settings.locale)}</strong>{active.reason === "afk" ? "No detectamos actividad" : "No pudimos clasificar este intervalo"}</span></div>
          <label className="field-label">¿Qué estabas haciendo?<input autoFocus value={activityLabel} onChange={(event) => setActivityLabel(event.target.value)} placeholder="Ej. Actualizar el reporte semanal" /></label>
          <div className="form-grid">
            <label className="field-label">Categoría<select value={category} onChange={(event) => setCategory(event.target.value as CategoryId)}>{CATEGORY_ORDER.map((item) => <option key={item} value={item}>{categoryLabel(settings.locale, item)}</option>)}</select></label>
            <label className="field-label">Proyecto, opcional<input value={project} onChange={(event) => setProject(event.target.value)} placeholder="Ej. Operación" /></label>
          </div>
          <label className="check-row"><input type="checkbox" checked={repetitive} onChange={(event) => setRepetitive(event.target.checked)} /><span><strong>Es una tarea repetitiva</strong><small>Ayuda a encontrar candidatos para automatizar.</small></span></label>
          <label className="check-row"><input type="checkbox" checked={rulesClear} onChange={(event) => setRulesClear(event.target.checked)} /><span><strong>Sigue reglas claras</strong><small>Hay poca decisión subjetiva en el proceso.</small></span></label>
          <button className="primary-button" disabled={!activityLabel.trim()}><Check size={17} />Guardar contexto</button>
        </form>
      ) : (
        <section className="empty-state"><Check size={30} /><h2>Todo tiene contexto.</h2><p>No hay preguntas pendientes. Puedes seguir con tu día.</p></section>
      )}
      {history.length > 0 && <section className="history-section"><div className="section-heading"><div><p className="eyebrow">Memoria reciente</p><h2>Respondidos</h2></div></div>{history.slice(0, 5).map((item) => <div className="history-row" key={item.id}><span><i style={{ background: categoryColor(item.category ?? "unknown") }} /><strong>{item.activityLabel}</strong><small>{item.project || categoryLabel(settings.locale, item.category ?? "unknown")}</small></span><time>{item.answeredAt ? formatTime(item.answeredAt, settings.locale) : "—"}</time></div>)}</section>}
    </>
  );
}

function StudyView({ progress, onDays }: { progress: StudyProgress; onDays: (days: 7 | 14 | 28) => void }) {
  const circumference = 2 * Math.PI * 54;
  const offset = circumference - circumference * Math.min(progress.coveragePercent, 100) / 100;
  return (
    <>
      <header className="page-header"><div><p className="eyebrow">Periodo de observación</p><h1>De actividad a evidencia.</h1><p className="page-intro">Mientras más días observes, más confiable será la recomendación de automatización.</p></div></header>
      <section className="study-hero">
        <div className="progress-ring"><svg viewBox="0 0 128 128" role="img" aria-label={`${progress.coveragePercent}% de cobertura`}><circle cx="64" cy="64" r="54" /><circle className="progress-value" cx="64" cy="64" r="54" style={{ strokeDasharray: circumference, strokeDashoffset: offset }} /></svg><span><b>{progress.observedDays.toFixed(1)}</b><small>de {progress.selectedDays} días</small></span></div>
        <div className="study-copy"><p className="eyebrow">Diagnóstico {progress.observedDays >= 7 ? "disponible" : "preliminar"}</p><h2>{progress.observedDays >= 7 ? "Ya hay una señal útil." : `Faltan ${(7 - progress.observedDays).toFixed(1)} días para una primera lectura.`}</h2><p>Tenemos {progress.trackedHours.toFixed(1)} horas observadas y {progress.answeredCheckIns} intervalos explicados por ti.</p><div className="study-metrics"><span><b>{progress.coveragePercent}%</b>cobertura</span><span><b>{progress.trackedHours.toFixed(1)} h</b>registradas</span><span><b>{progress.answeredCheckIns}</b>respuestas</span></div></div>
      </section>
      <section className="period-section"><div><p className="eyebrow">Profundidad</p><h2>Elige el periodo de análisis</h2></div><div className="segmented-control">{([7, 14, 28] as const).map((days) => <button key={days} className={progress.selectedDays === days ? "active" : ""} onClick={() => onDays(days)}><b>{days} días</b><small>{days === 7 ? "Primera señal" : days === 14 ? "Patrones estables" : "Lectura profunda"}</small></button>)}</div></section>
    </>
  );
}

function AutomateView({ diagnostic, onGenerate, onCopy, onOpen }: { diagnostic: DiagnosticV1; onGenerate: () => Promise<void>; onCopy: () => Promise<void>; onOpen: (url: string) => Promise<void> }) {
  const candidate = diagnostic.candidates[0];
  return (
    <>
      <header className="page-header"><div><p className="eyebrow">Diagnóstico personal</p><h1>Qué automatizar primero.</h1><p className="page-intro">Una recomendación basada en lo que realmente repites, con supuestos visibles.</p></div><button className="secondary-button" onClick={() => void onGenerate()}><RotateCcw size={16} />Actualizar</button></header>
      {diagnostic.preliminary && <div className="preliminary-banner"><Gauge size={18} /><span><strong>Lectura preliminar</strong>Hay {diagnostic.daysObserved.toFixed(1)} días observados. Úsala como hipótesis, no como veredicto.</span></div>}
      {candidate ? (
        <section className="candidate-section">
          <div className="candidate-rank"><span>01</span><small>Prioridad</small></div>
          <div className="candidate-main"><p className="eyebrow">Primer candidato</p><h2>{candidate.activityLabel}</h2><p>{candidate.evidence.join(" · ")}</p><div className="score-line"><span><b>{candidate.priorityScore}</b>/100 señal</span><span><b>{candidate.estimatedWeeklyHours}</b> h/semana</span><span><b>{candidate.weeklyFrequency}</b> repeticiones</span></div></div>
          <div className="candidate-scores"><span><b>{candidate.impactScore}/40</b>Impacto</span><span><b>{candidate.feasibilityScore}/35</b>Facilidad</span><span><b>{candidate.riskScore}/25</b>Riesgo</span></div>
        </section>
      ) : <section className="empty-state"><Target size={30} /><h2>Aún no hay un candidato sólido.</h2><p>Marca tareas repetitivas en tus check-ins para formar una señal.</p></section>}
      <section className="recommendations"><div><p className="eyebrow">Siguiente movimiento</p><h2>Recomendaciones</h2></div><ol>{diagnostic.recommendations.map((recommendation, index) => <li key={recommendation}><span>{String(index + 1).padStart(2, "0")}</span><p>{recommendation}</p></li>)}</ol></section>
      <section className="prompt-section">
        <div className="prompt-heading"><div><p className="eyebrow">Lleva el diagnóstico contigo</p><h2>Prompt listo para conversar</h2></div><button className="primary-button" onClick={() => void onCopy()}><Clipboard size={16} />Copiar prompt</button></div>
        <pre>{diagnostic.prompt}</pre>
        <div className="prompt-links"><button onClick={() => void onOpen("https://chatgpt.com/")}>Abrir ChatGPT <ArrowUpRight size={15} /></button><button onClick={() => void onOpen("https://claude.ai/new")}>Abrir Claude <ArrowUpRight size={15} /></button><span><LockKeyhole size={14} />Solo sale lo que tú copies</span></div>
      </section>
    </>
  );
}

function SettingsView({ settings, status, onSave, onExport, onDelete }: { settings: SettingsV1; status: TrackingStatus; onSave: (value: SettingsV1) => Promise<void>; onExport: (format: "json" | "csv") => Promise<void>; onDelete: () => Promise<void> }) {
  const [draft, setDraft] = useState(settings);
  useEffect(() => setDraft(settings), [settings]);
  const set = <K extends keyof SettingsV1>(key: K, value: SettingsV1[K]) => setDraft((current) => ({ ...current, [key]: value }));
  return (
    <>
      <header className="page-header"><div><p className="eyebrow">Control local</p><h1>Configuración.</h1><p className="page-intro">Decide cuánto registrar, cuándo preguntar y por cuánto tiempo conservarlo.</p></div><button className="primary-button" onClick={() => void onSave(draft)}><Check size={16} />Guardar cambios</button></header>
      <section className="settings-group"><div className="settings-intro"><Clock3 size={19} /><div><h2>Seguimiento</h2><p>Frecuencia y horario del estudio.</p></div></div><div className="settings-fields"><label>Cadencia<select value={draft.cadenceMinutes} onChange={(event) => set("cadenceMinutes", Number(event.target.value) as 15 | 30 | 60)}><option value={15}>Cada 15 minutos</option><option value={30}>Cada 30 minutos</option><option value={60}>Cada 60 minutos</option></select></label><label>Conservar datos<select value={draft.retentionDays ?? "forever"} onChange={(event) => set("retentionDays", event.target.value === "forever" ? null : Number(event.target.value) as 30 | 90 | 365)}><option value={30}>30 días</option><option value={90}>90 días</option><option value={365}>1 año</option><option value="forever">Sin límite</option></select></label><label className="toggle-row"><span><strong>Iniciar con el sistema</strong><small>Activa el registro sin abrir la ventana.</small></span><input type="checkbox" checked={draft.autostart} onChange={(event) => set("autostart", event.target.checked)} /></label><label className="toggle-row"><span><strong>Guardar títulos de ventana</strong><small>Mejora el contexto; puede incluir nombres de documentos.</small></span><input type="checkbox" checked={draft.captureWindowTitles} onChange={(event) => set("captureWindowTitles", event.target.checked)} /></label></div></section>
      <section className="settings-group"><div className="settings-intro"><Languages size={19} /><div><h2>Apariencia e idioma</h2><p>La interfaz cambia; tus datos no.</p></div></div><div className="settings-fields two-columns"><label>Idioma<select value={draft.locale} onChange={(event) => set("locale", event.target.value as SettingsV1["locale"])}><option value="es-419">Español (Latinoamérica)</option><option value="en">English</option></select></label><label>Tema<select value={draft.theme} onChange={(event) => set("theme", event.target.value as SettingsV1["theme"])}><option value="ink">Tinta</option><option value="paper">Papel</option><option value="system">Sistema</option></select></label></div></section>
      <section className="settings-group"><div className="settings-intro"><LockKeyhole size={19} /><div><h2>Datos</h2><p>{status.serverPort ? `Motor local en 127.0.0.1:${status.serverPort}.` : "Motor local; sin nube ni telemetría."}</p></div></div><div className="data-actions"><button className="secondary-button" onClick={() => void onExport("json")}><Download size={16} />Exportar JSON</button><button className="secondary-button" onClick={() => void onExport("csv")}><Download size={16} />Exportar CSV</button><button className="danger-button" onClick={() => void onDelete()}><Trash2 size={16} />Borrar todo</button></div></section>
    </>
  );
}

export default function App() {
  const [view, setView] = useState<View>("today");
  const [loading, setLoading] = useState(true);
  const [settings, setSettings] = useState<SettingsV1 | null>(null);
  const [status, setStatus] = useState<TrackingStatus>(EMPTY_STATUS);
  const [blocks, setBlocks] = useState<TimelineBlockV1[]>([]);
  const [checkIns, setCheckIns] = useState<CheckInV1[]>([]);
  const [study, setStudy] = useState<StudyProgress | null>(null);
  const [diagnostic, setDiagnostic] = useState<DiagnosticV1 | null>(null);
  const [notice, setNotice] = useState<string>();

  async function load() {
    const from = new Date(); from.setHours(0, 0, 0, 0);
    const to = new Date(); to.setHours(23, 59, 59, 999);
    const [nextSettings, nextStatus, nextBlocks, nextCheckIns, nextStudy] = await Promise.all([
      bridge.getSettings(), bridge.getTrackingStatus(), bridge.getTimeline(from.toISOString(), to.toISOString()), bridge.listCheckIns(), bridge.getStudyProgress(),
    ]);
    setSettings(nextSettings); setStatus(nextStatus); setBlocks(nextBlocks); setCheckIns(nextCheckIns); setStudy(nextStudy);
    setDiagnostic(await bridge.generateDiagnostic(nextStudy.selectedDays));
    setLoading(false);
  }

  useEffect(() => { void load(); }, []);
  useEffect(() => {
    if (!settings) return;
    const theme = settings.theme === "system" ? (matchMedia("(prefers-color-scheme: light)").matches ? "paper" : "ink") : settings.theme;
    document.documentElement.dataset.theme = theme;
  }, [settings]);
  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(undefined), 2600);
    return () => window.clearTimeout(timer);
  }, [notice]);

  if (loading || !settings || !study || !diagnostic) return <main className="loading-screen"><BrandMark /><span className="loading-line" /><p>Organizando tu tiempo…</p></main>;
  if (!settings.onboardingCompleted) return <Onboarding initial={settings} onComplete={async (value) => { setSettings(await bridge.saveSettings(value)); }} />;

  const nav = messages(settings.locale).nav;
  const items: { id: View; label: string; icon: typeof Activity }[] = [
    { id: "today", label: nav.today, icon: Activity }, { id: "checkins", label: nav.checkins, icon: HelpCircle }, { id: "study", label: nav.study, icon: Focus }, { id: "automate", label: nav.automate, icon: Sparkles }, { id: "settings", label: nav.settings, icon: SettingsIcon },
  ];
  const saveSettings = async (value: SettingsV1) => { setSettings(await bridge.saveSettings(value)); setNotice("Configuración guardada"); };
  const answer = async (value: CheckInAnswer) => { await bridge.answerCheckIn(value); setCheckIns(await bridge.listCheckIns()); setStatus(await bridge.getTrackingStatus()); setNotice("Contexto guardado"); };
  const selectDays = (days: 7 | 14 | 28) => { setStudy({ ...study, selectedDays: days }); void bridge.generateDiagnostic(days).then(setDiagnostic); };

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <BrandMark />
        <nav aria-label="Principal">{items.map(({ id, label, icon: Icon }) => <button key={id} className={view === id ? "active" : ""} onClick={() => setView(id)}><Icon size={18} /><span>{label}</span>{id === "checkins" && status.pendingCheckIns > 0 && <b>{status.pendingCheckIns}</b>}</button>)}</nav>
        <div className="sidebar-footer"><StatusPill status={status} /><p><LockKeyhole size={13} />Solo en este equipo</p><small>Ensō Time v0.1 beta</small></div>
      </aside>
      <main className="content">
        {view === "today" && <TodayView settings={settings} status={status} blocks={blocks} checkIns={checkIns} onNavigate={setView} onPause={async () => { setStatus(await bridge.pauseTracking(30)); setNotice("Registro pausado por 30 minutos"); }} onResume={async () => { setStatus(await bridge.resumeTracking()); setNotice("Registro reanudado"); }} />}
        {view === "checkins" && <CheckInsView checkIns={checkIns} settings={settings} onAnswer={answer} />}
        {view === "study" && <StudyView progress={study} onDays={selectDays} />}
        {view === "automate" && <AutomateView diagnostic={diagnostic} onGenerate={async () => { setDiagnostic(await bridge.generateDiagnostic(study.selectedDays)); setNotice("Diagnóstico actualizado"); }} onCopy={async () => { await navigator.clipboard.writeText(diagnostic.prompt); setNotice("Prompt copiado"); }} onOpen={bridge.openExternal} />}
        {view === "settings" && <SettingsView settings={settings} status={status} onSave={saveSettings} onExport={async (format) => { const path = await bridge.exportData(format); setNotice(`Exportado en ${path}`); }} onDelete={async () => { if (!window.confirm("¿Borrar permanentemente toda la información de Ensō Time?")) return; await bridge.deleteAllData(); setNotice("Datos eliminados"); await load(); }} />}
      </main>
      {notice && <div className="toast" role="status"><Check size={15} />{notice}</div>}
    </div>
  );
}
