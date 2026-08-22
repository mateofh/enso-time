import type { CategoryId, Locale } from "./types";

interface UiCopy {
  nav: Record<"today" | "checkins" | "study" | "automate" | "settings", string>;
  common: {
    local: string;
    loading: string;
    retry: string;
    running: string;
    paused: string;
    saved: string;
    unexpectedError: string;
  };
  onboarding: {
    progress: (step: number) => string;
    steps: Array<{ label: string; title: string; copy: string }>;
    roleLabel: string;
    rolePlaceholder: string;
    goalLabel: string;
    goalPlaceholder: string;
    titlesConsent: string;
    titlesConsentHint: string;
    back: string;
    continue: string;
    finish: string;
  };
  today: {
    dateLabel: string;
    title: string;
    observed: (duration: string) => string;
    pause: string;
    resume: string;
    missingTitle: string;
    missingCopy: (start: string, end: string) => string;
    timeline: string;
    timelineHint: string;
    emptyTimeline: string;
    distribution: string;
    distributionHint: string;
    privateTitle: string;
    privateCopy: string;
  };
  checkins: {
    title: string;
    intro: string;
    pending: (count: number) => string;
    afk: string;
    ambiguous: string;
    activity: string;
    activityPlaceholder: string;
    category: string;
    project: string;
    projectPlaceholder: string;
    repetitive: string;
    repetitiveHint: string;
    rules: string;
    rulesHint: string;
    save: string;
    allDone: string;
    allDoneCopy: string;
    history: string;
  };
  study: {
    title: string;
    intro: string;
    available: string;
    preliminary: string;
    usefulSignal: string;
    remaining: (days: string) => string;
    evidence: (hours: string, answers: number) => string;
    coverage: string;
    tracked: string;
    answers: string;
    periodTitle: string;
    periods: Record<7 | 14 | 28, string>;
    days: string;
  };
  automate: {
    title: string;
    intro: string;
    refresh: string;
    preliminaryTitle: string;
    preliminaryCopy: (days: string) => string;
    candidate: string;
    signal: string;
    weeklyHours: string;
    repetitions: string;
    impact: string;
    feasibility: string;
    risk: string;
    empty: string;
    emptyCopy: string;
    recommendations: string;
    promptTitle: string;
    promptIntro: string;
    copyPrompt: string;
    openChatGpt: string;
    openClaude: string;
    outboundNotice: string;
  };
  settings: {
    title: string;
    intro: string;
    save: string;
    tracking: string;
    trackingHint: string;
    cadence: string;
    cadenceOption: (minutes: number) => string;
    retention: string;
    retention30: string;
    retention90: string;
    retention365: string;
    retentionForever: string;
    autostart: string;
    autostartHint: string;
    titles: string;
    titlesHint: string;
    schedule: string;
    scheduleHint: string;
    start: string;
    end: string;
    appearance: string;
    appearanceHint: string;
    language: string;
    theme: string;
    ink: string;
    paper: string;
    system: string;
    updates: string;
    updatesHint: string;
    data: string;
    localEngine: (port?: number) => string;
    exportJson: string;
    exportCsv: string;
    delete: string;
    deleteConfirm: string;
  };
  notices: {
    settingsSaved: string;
    contextSaved: string;
    paused: string;
    resumed: string;
    diagnosticUpdated: string;
    promptCopied: string;
    exported: (path: string) => string;
    deleted: string;
  };
}

const categories = {
  "es-419": {
    focus: "Trabajo enfocado",
    communication: "Comunicación",
    meetings: "Reuniones",
    admin: "Administración",
    learning: "Aprendizaje",
    creation: "Creación",
    personal: "Personal",
    break: "Descanso",
    unknown: "Sin clasificar",
  },
  en: {
    focus: "Focused work",
    communication: "Communication",
    meetings: "Meetings",
    admin: "Admin",
    learning: "Learning",
    creation: "Creation",
    personal: "Personal",
    break: "Break",
    unknown: "Unclassified",
  },
} satisfies Record<Locale, Record<CategoryId, string>>;

const copy: Record<Locale, UiCopy> = {
  "es-419": {
    nav: { today: "Hoy", checkins: "Check-ins", study: "Estudio", automate: "Automatizar", settings: "Configuración" },
    common: { local: "Solo en este equipo", loading: "Organizando tu tiempo…", retry: "Intentar de nuevo", running: "Registrando", paused: "En pausa", saved: "Guardado", unexpectedError: "No pudimos completar la acción. Intenta de nuevo." },
    onboarding: {
      progress: (step) => `Paso ${step} de 3`,
      steps: [
        { label: "Tu tiempo, en privado", title: "Entiende tu día sin llenar cronómetros.", copy: "Ensō Time observa aplicaciones activas en este computador y solo pregunta cuando falta contexto. Los datos no salen de tu equipo." },
        { label: "Una pregunta breve", title: "Completa únicamente los intervalos ambiguos.", copy: "Cada 15 minutos revisamos si la actividad quedó clara. Si lo está, no interrumpimos. Si no, puedes responder después." },
        { label: "Automatiza con evidencia", title: "Convierte patrones repetidos en una decisión.", copy: "Después de 7, 14 o 28 días recibirás un diagnóstico y un prompt listo para analizar tu primera automatización." },
      ],
      roleLabel: "¿Cuál es tu rol?", rolePlaceholder: "Ej. Operador de negocio digital", goalLabel: "¿Qué tiempo quieres recuperar?", goalPlaceholder: "Ej. Reducir reportes manuales", titlesConsent: "Guardar títulos de ventana", titlesConsentHint: "Opcional. Mejora el contexto, pero puede incluir nombres de documentos, pestañas o clientes. Todo permanece local.", back: "Atrás", continue: "Continuar", finish: "Comenzar estudio",
    },
    today: {
      dateLabel: "Resumen de hoy", title: "Así se ve tu día.", observed: (duration) => `${duration} observados · la información sigue en este equipo.`, pause: "Pausar 30 min", resume: "Reanudar", missingTitle: "Falta contexto en un intervalo", missingCopy: (start, end) => `${start}–${end} · responder toma menos de 20 segundos`, timeline: "Línea de tiempo", timelineHint: "Registro continuo", emptyTimeline: "Aún no hay actividad registrada hoy.", distribution: "En qué se fue", distributionHint: "Distribución", privateTitle: "Privado por diseño", privateCopy: "Aplicaciones y títulos de ventana permanecen locales.",
    },
    checkins: {
      title: "Completa los huecos.", intro: "Ensō Time pregunta solo cuando las aplicaciones no cuentan toda la historia.", pending: (count) => count === 1 ? "1 intervalo pendiente" : `${count} intervalos pendientes`, afk: "No detectamos actividad", ambiguous: "No pudimos clasificar este intervalo", activity: "¿Qué estabas haciendo?", activityPlaceholder: "Ej. Actualizar el reporte semanal", category: "Categoría", project: "Proyecto, opcional", projectPlaceholder: "Ej. Operación", repetitive: "Es una tarea repetitiva", repetitiveHint: "Ayuda a encontrar candidatos para automatizar.", rules: "Sigue reglas claras", rulesHint: "Hay poca decisión subjetiva en el proceso.", save: "Guardar contexto", allDone: "Todo tiene contexto.", allDoneCopy: "No hay preguntas pendientes. Puedes seguir con tu día.", history: "Respondidos recientemente",
    },
    study: {
      title: "De actividad a evidencia.", intro: "Mientras más días observes, más confiable será la recomendación de automatización.", available: "Diagnóstico disponible", preliminary: "Diagnóstico preliminar", usefulSignal: "Ya hay una señal útil.", remaining: (days) => `Faltan ${days} días para una primera lectura.`, evidence: (hours, answers) => `Tenemos ${hours} horas observadas y ${answers} intervalos explicados por ti.`, coverage: "cobertura", tracked: "registradas", answers: "respuestas", periodTitle: "Elige el periodo de análisis", periods: { 7: "Primera señal", 14: "Patrones estables", 28: "Lectura profunda" }, days: "días",
    },
    automate: {
      title: "Qué automatizar primero.", intro: "Una recomendación basada en lo que realmente repites, con supuestos visibles.", refresh: "Actualizar", preliminaryTitle: "Lectura preliminar", preliminaryCopy: (days) => `Hay ${days} días observados. Úsala como hipótesis, no como veredicto.`, candidate: "Primer candidato", signal: "señal", weeklyHours: "h/semana", repetitions: "repeticiones", impact: "Impacto", feasibility: "Facilidad", risk: "Riesgo", empty: "Aún no hay un candidato sólido.", emptyCopy: "Marca tareas repetitivas en tus check-ins para formar una señal.", recommendations: "Recomendaciones", promptTitle: "Prompt listo para conversar", promptIntro: "Lleva el diagnóstico contigo", copyPrompt: "Copiar prompt", openChatGpt: "Abrir ChatGPT", openClaude: "Abrir Claude", outboundNotice: "Solo sale lo que tú copies",
    },
    settings: {
      title: "Configuración.", intro: "Decide cuánto registrar, cuándo preguntar y por cuánto tiempo conservarlo.", save: "Guardar cambios", tracking: "Seguimiento", trackingHint: "Frecuencia y horario del estudio.", cadence: "Cadencia", cadenceOption: (minutes) => `Cada ${minutes} minutos`, retention: "Conservar datos", retention30: "30 días", retention90: "90 días", retention365: "1 año", retentionForever: "Sin límite", autostart: "Iniciar con el sistema", autostartHint: "Activa el registro sin abrir la ventana.", titles: "Guardar títulos de ventana", titlesHint: "Mejora el contexto; puede incluir nombres de documentos.", schedule: "Horario de trabajo", scheduleHint: "Los check-ins se mantienen dentro de esta franja.", start: "Inicio", end: "Fin", appearance: "Apariencia e idioma", appearanceHint: "La interfaz cambia; tus datos no.", language: "Idioma", theme: "Tema", ink: "Tinta", paper: "Papel", system: "Sistema", updates: "Buscar nuevas versiones", updatesHint: "Consulta GitHub al abrir la aplicación; no envía actividad.", data: "Datos", localEngine: (port) => port ? `Motor local en 127.0.0.1:${port}.` : "Motor local; sin nube ni telemetría.", exportJson: "Exportar JSON", exportCsv: "Exportar CSV", delete: "Borrar todo", deleteConfirm: "¿Borrar permanentemente toda la información de Ensō Time? Esta acción no se puede deshacer.",
    },
    notices: { settingsSaved: "Configuración guardada", contextSaved: "Contexto guardado", paused: "Registro pausado por 30 minutos", resumed: "Registro reanudado", diagnosticUpdated: "Diagnóstico actualizado", promptCopied: "Prompt copiado", exported: (path) => `Exportado en ${path}`, deleted: "Datos eliminados" },
  },
  en: {
    nav: { today: "Today", checkins: "Check-ins", study: "Study", automate: "Automate", settings: "Settings" },
    common: { local: "Only on this device", loading: "Organizing your time…", retry: "Try again", running: "Tracking", paused: "Paused", saved: "Saved", unexpectedError: "We couldn't complete that action. Try again." },
    onboarding: {
      progress: (step) => `Step ${step} of 3`,
      steps: [
        { label: "Your time, kept private", title: "Understand your day without running timers.", copy: "Ensō Time observes active apps on this computer and only asks when context is missing. Your data never leaves this device." },
        { label: "One short question", title: "Fill in only the ambiguous intervals.", copy: "Every 15 minutes, we check whether the activity is clear. If it is, we stay quiet. If not, you can answer later." },
        { label: "Automate with evidence", title: "Turn repeated patterns into a decision.", copy: "After 7, 14, or 28 days, you'll get a diagnosis and a ready-to-use prompt for your first automation." },
      ],
      roleLabel: "What is your role?", rolePlaceholder: "E.g. Digital operations lead", goalLabel: "What time do you want to recover?", goalPlaceholder: "E.g. Reduce manual reporting", titlesConsent: "Save window titles", titlesConsentHint: "Optional. This improves context, but may include document, tab, or client names. Everything stays local.", back: "Back", continue: "Continue", finish: "Start study",
    },
    today: {
      dateLabel: "Today's summary", title: "This is how your day looks.", observed: (duration) => `${duration} observed · the information stays on this device.`, pause: "Pause for 30 min", resume: "Resume", missingTitle: "One interval needs context", missingCopy: (start, end) => `${start}–${end} · answering takes less than 20 seconds`, timeline: "Timeline", timelineHint: "Continuous record", emptyTimeline: "No activity has been recorded today yet.", distribution: "Where it went", distributionHint: "Distribution", privateTitle: "Private by design", privateCopy: "App names and window titles remain local.",
    },
    checkins: {
      title: "Fill in the gaps.", intro: "Ensō Time only asks when apps don't tell the whole story.", pending: (count) => count === 1 ? "1 pending interval" : `${count} pending intervals`, afk: "No activity was detected", ambiguous: "We couldn't classify this interval", activity: "What were you doing?", activityPlaceholder: "E.g. Updating the weekly report", category: "Category", project: "Project, optional", projectPlaceholder: "E.g. Operations", repetitive: "This is a repetitive task", repetitiveHint: "Helps identify automation candidates.", rules: "It follows clear rules", rulesHint: "There is little subjective judgment in the process.", save: "Save context", allDone: "Everything has context.", allDoneCopy: "There are no pending questions. You can get back to your day.", history: "Recently answered",
    },
    study: {
      title: "From activity to evidence.", intro: "The more days you observe, the more reliable the automation recommendation becomes.", available: "Diagnosis available", preliminary: "Preliminary diagnosis", usefulSignal: "There is a useful signal now.", remaining: (days) => `${days} days until the first reading.`, evidence: (hours, answers) => `We have ${hours} observed hours and ${answers} intervals explained by you.`, coverage: "coverage", tracked: "tracked", answers: "answers", periodTitle: "Choose the analysis period", periods: { 7: "First signal", 14: "Stable patterns", 28: "Deep reading" }, days: "days",
    },
    automate: {
      title: "What to automate first.", intro: "A recommendation based on what you actually repeat, with visible assumptions.", refresh: "Refresh", preliminaryTitle: "Preliminary reading", preliminaryCopy: (days) => `${days} days have been observed. Use this as a hypothesis, not a verdict.`, candidate: "Top candidate", signal: "signal", weeklyHours: "hrs/week", repetitions: "repetitions", impact: "Impact", feasibility: "Feasibility", risk: "Risk", empty: "There isn't a strong candidate yet.", emptyCopy: "Mark repetitive tasks in your check-ins to build a signal.", recommendations: "Recommendations", promptTitle: "A ready-to-use prompt", promptIntro: "Take the diagnosis with you", copyPrompt: "Copy prompt", openChatGpt: "Open ChatGPT", openClaude: "Open Claude", outboundNotice: "Only what you copy leaves the app",
    },
    settings: {
      title: "Settings.", intro: "Choose how much to track, when to ask, and how long to keep it.", save: "Save changes", tracking: "Tracking", trackingHint: "Study frequency and working hours.", cadence: "Cadence", cadenceOption: (minutes) => `Every ${minutes} minutes`, retention: "Keep data", retention30: "30 days", retention90: "90 days", retention365: "1 year", retentionForever: "No limit", autostart: "Launch at startup", autostartHint: "Starts tracking without opening the window.", titles: "Save window titles", titlesHint: "Improves context; may include document names.", schedule: "Working hours", scheduleHint: "Check-ins stay within this time window.", start: "Start", end: "End", appearance: "Appearance and language", appearanceHint: "The interface changes; your data does not.", language: "Language", theme: "Theme", ink: "Ink", paper: "Paper", system: "System", updates: "Check for new versions", updatesHint: "Checks GitHub when the app opens; no activity data is sent.", data: "Data", localEngine: (port) => port ? `Local engine on 127.0.0.1:${port}.` : "Local engine; no cloud or telemetry.", exportJson: "Export JSON", exportCsv: "Export CSV", delete: "Delete everything", deleteConfirm: "Permanently delete all Ensō Time data? This cannot be undone.",
    },
    notices: { settingsSaved: "Settings saved", contextSaved: "Context saved", paused: "Tracking paused for 30 minutes", resumed: "Tracking resumed", diagnosticUpdated: "Diagnosis refreshed", promptCopied: "Prompt copied", exported: (path) => `Exported to ${path}`, deleted: "Data deleted" },
  },
};

export function messages(locale: Locale): UiCopy {
  return copy[locale];
}

export function categoryLabel(locale: Locale, category: CategoryId): string {
  return categories[locale][category];
}

export function formatDuration(seconds: number, locale: Locale): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.round((seconds % 3600) / 60);
  if (hours === 0) return locale === "en" ? `${minutes}m` : `${minutes} min`;
  return locale === "en" ? `${hours}h ${minutes}m` : `${hours} h ${minutes} min`;
}

export function formatTime(iso: string, locale: Locale): string {
  return new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(new Date(iso));
}
