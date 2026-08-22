import type { CaptureIssue, CategoryId, Locale } from "./types";

interface UiCopy {
  nav: Record<"today" | "checkins" | "study" | "automate" | "settings", string>;
  common: {
    local: string;
    loading: string;
    retry: string;
    running: string;
    paused: string;
    degraded: string;
    saved: string;
    unexpectedError: string;
  };
  captureIssue: {
    label: string;
    title: string;
    retry: string;
    details: Record<CaptureIssue, string>;
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
    workflowAction: string;
    workflowHint: string;
    openChatGpt: string;
    openClaude: string;
    outboundNotice: string;
  };
  workflow: {
    context: string;
    title: string;
    intro: string;
    candidate: string;
    back: string;
    loading: string;
    retry: string;
    emptyTitle: string;
    emptyCopy: string;
    errorTitle: string;
    loadError: string;
    saveError: string;
    validation: string;
    savedTitle: string;
    savedCopy: string;
    startTitle: string;
    startHint: string;
    trigger: string;
    triggerPlaceholder: string;
    frequency: string;
    frequencyPlaceholder: string;
    minutesPerRun: string;
    toolsTitle: string;
    toolsHint: string;
    tool: (index: number) => string;
    toolPlaceholder: string;
    addTool: string;
    removeTool: (index: number) => string;
    stepsTitle: string;
    stepsHint: string;
    step: (index: number) => string;
    stepPlaceholder: string;
    addStep: string;
    removeStep: (index: number) => string;
    contextTitle: string;
    contextHint: string;
    humanDecisions: string;
    humanDecisionsPlaceholder: string;
    inputs: string;
    inputsPlaceholder: string;
    desiredOutput: string;
    desiredOutputPlaceholder: string;
    painPoints: string;
    painPointsPlaceholder: string;
    localTitle: string;
    localCopy: string;
    save: string;
    saving: string;
  };
  blueprint: BlueprintCopy;
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

interface BlueprintCopy {
  context: string; title: string; intro: string; back: string; loading: string; retry: string; emptyTitle: string; emptyCopy: string; mapTitle: string; mapCopy: string; mapAction: string; errorTitle: string;
  status: string; complete: string; statuses: Record<"draft" | "ready" | "stale" | "blocked", string>;
  staleTitle: string; staleCopy: string; refresh: string; refreshed: string; blockedTitle: string; blockedCopy: string;
  evidenceLabel: string; sourceTitle: string; sourceHint: string; trigger: string; inputs: string; output: string; tools: string; observed: string; declared: string;
  frequencyTitle: string; frequencyHint: string; runsPerWeek: string; boundariesTitle: string; boundariesHint: string; disposition: string; rationale: string;
  dispositions: Record<"unassessed" | "automatable" | "human-required", string>;
  safetyTitle: string; safetyHint: string; sensitivity: string; sideEffects: string; verification: string; verificationMinutes: string; recovery: string;
  sensitivities: Record<"unassessed" | "low" | "moderate" | "high", string>;
  sideEffectLevels: Record<"unassessed" | "none" | "reversible" | "irreversible", string>;
  verificationModes: Record<"unassessed" | "none" | "sample" | "every-run", string>;
  simulation: string; estimateTitle: string; assumptions: string; baseline: string; residual: string; savings: string; effort: string; risk: string; confidence: string;
  localTitle: string; localCopy: string; save: string; saving: string; saved: string; copy: string; copied: string; exportTitle: string; exportHint: string; exportMarkdown: string; exportJson: string; exported: (path: string) => string; deletePreview: string; deleteTitle: string; deleteCopy: string; cancel: string;
  errors: Record<string, string>;
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
    common: { local: "Solo en este equipo", loading: "Organizando tu tiempo…", retry: "Intentar de nuevo", running: "Registrando", paused: "En pausa", degraded: "Registro incompleto", saved: "Guardado", unexpectedError: "No pudimos completar la acción. Intenta de nuevo." },
    captureIssue: {
      label: "Atención del registro",
      title: "Ensō Time no está registrando todo.",
      retry: "Volver a comprobar",
      details: {
        "server-unavailable": "El motor local no responde. Reinicia Ensō Time y vuelve a comprobar; tu información guardada permanece en este equipo.",
        "window-watcher-unavailable": "No podemos ver qué aplicación está activa. Revisa los permisos de Ensō Time en el sistema y vuelve a comprobar.",
        "afk-watcher-unavailable": "No podemos distinguir cuándo te ausentas. Mantén Ensō Time abierto y vuelve a comprobar para recuperar ese contexto.",
        "watcher-conflict": "Hay otra sesión intentando registrar este equipo. Cierra la sesión duplicada y vuelve a comprobar.",
      },
    },
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
      title: "Qué automatizar primero.", intro: "Una recomendación basada en lo que realmente repites, con supuestos visibles.", refresh: "Actualizar", preliminaryTitle: "Lectura preliminar", preliminaryCopy: (days) => `Hay ${days} días observados. Úsala como hipótesis, no como veredicto.`, candidate: "Primer candidato", signal: "señal", weeklyHours: "h/semana", repetitions: "repeticiones", impact: "Impacto", feasibility: "Facilidad", risk: "Riesgo", empty: "Aún no hay un candidato sólido.", emptyCopy: "Marca tareas repetitivas en tus check-ins para formar una señal.", recommendations: "Recomendaciones", promptTitle: "Prompt listo para conversar", promptIntro: "Lleva el diagnóstico contigo", copyPrompt: "Copiar prompt", workflowAction: "Levantar workflow", workflowHint: "Documenta el proceso antes de diseñar la automatización.", openChatGpt: "Abrir ChatGPT", openClaude: "Abrir Claude", outboundNotice: "Solo sale lo que tú copies",
    },
    workflow: {
      context: "Levantamiento de workflow",
      title: "Describe cómo ocurre hoy.",
      intro: "Captura el proceso real, incluidos sus criterios humanos y fricciones. Aquí no se genera ninguna automatización.",
      candidate: "Candidato",
      back: "Volver al diagnóstico",
      loading: "Buscando un levantamiento guardado…",
      retry: "Intentar de nuevo",
      emptyTitle: "No hay un candidato para levantar.",
      emptyCopy: "Vuelve al diagnóstico y espera una señal con evidencia antes de documentar el workflow.",
      errorTitle: "No pudimos abrir el levantamiento.",
      loadError: "No pudimos consultar los levantamientos guardados.",
      saveError: "No pudimos guardar el levantamiento.",
      validation: "Completa los campos requeridos antes de guardar.",
      savedTitle: "Levantamiento guardado",
      savedCopy: "Puedes volver, editarlo o continuar después. El diagnóstico ya usa este contexto.",
      startTitle: "Inicio y frecuencia",
      startHint: "Define qué pone el proceso en marcha y cuánto cuesta cada repetición.",
      trigger: "¿Qué dispara este workflow?",
      triggerPlaceholder: "Ej. Llega el cierre semanal del equipo comercial",
      frequency: "Frecuencia",
      frequencyPlaceholder: "Ej. Cada viernes",
      minutesPerRun: "Minutos por ejecución",
      toolsTitle: "Herramientas",
      toolsHint: "Incluye cada aplicación, archivo o canal que participa.",
      tool: (index) => `Herramienta ${index}`,
      toolPlaceholder: "Ej. Google Sheets",
      addTool: "Agregar herramienta",
      removeTool: (index) => `Quitar herramienta ${index}`,
      stepsTitle: "Pasos en orden",
      stepsHint: "Describe lo que sucede, un paso observable a la vez.",
      step: (index) => `Paso ${index}`,
      stepPlaceholder: "Ej. Exportar las oportunidades cerradas",
      addStep: "Agregar paso",
      removeStep: (index) => `Quitar paso ${index}`,
      contextTitle: "Criterio y resultado",
      contextHint: "Separa los datos de entrada, las decisiones humanas y el resultado esperado.",
      humanDecisions: "Decisiones humanas",
      humanDecisionsPlaceholder: "Ej. Revisar excepciones y decidir qué cuentas excluir",
      inputs: "Entradas",
      inputsPlaceholder: "Ej. CSV de ventas, tabla de metas y notas del CRM",
      desiredOutput: "Resultado deseado",
      desiredOutputPlaceholder: "Ej. Reporte validado y listo para compartir",
      painPoints: "Puntos de dolor",
      painPointsPlaceholder: "Ej. Copiar datos, corregir formatos y perseguir campos faltantes",
      localTitle: "Guardado local",
      localCopy: "Este levantamiento permanece en Ensō Time y no crea flujos en servicios externos.",
      save: "Guardar levantamiento",
      saving: "Guardando…",
    },
    blueprint: {
      context: "Blueprint de automatización", title: "Del proceso a límites claros.", intro: "Especifica qué puede automatizarse y qué debe seguir bajo control humano. No conectamos cuentas ni ejecutamos flujos.", back: "Volver al diagnóstico", loading: "Preparando el blueprint…", retry: "Intentar de nuevo", emptyTitle: "No hay un candidato para continuar.", emptyCopy: "Vuelve al diagnóstico para elegir una recomendación con evidencia.", mapTitle: "Primero mapea el workflow.", mapCopy: "El blueprint necesita un levantamiento del proceso actual como fuente de solo lectura.", mapAction: "Mapear workflow", errorTitle: "No pudimos abrir el blueprint.",
      status: "Estado", complete: "completo", statuses: { draft: "Borrador", ready: "Listo para construir", stale: "Fuente desactualizada", blocked: "Bloqueado" }, staleTitle: "El levantamiento cambió.", staleCopy: "Actualiza la fuente antes de guardar, copiar o exportar. Las clasificaciones de pasos se reiniciarán.", refresh: "Actualizar fuente", refreshed: "Fuente actualizada; revisa los límites de cada paso.", blockedTitle: "Este blueprint aún no puede continuar.", blockedCopy: "Completa la información faltante o resuelve el bloqueo de la fuente.",
      evidenceLabel: "Evidencia de solo lectura", sourceTitle: "Proceso actual", sourceHint: "La fuente y la evidencia observada no se pueden editar desde este blueprint.", trigger: "Disparador", inputs: "Entradas", output: "Resultado", tools: "Herramientas", observed: "Observado", declared: "Declarado por ti",
      frequencyTitle: "Frecuencia", frequencyHint: "Solo falta expresar cuántas veces ocurre en una semana típica.", runsPerWeek: "Frecuencia semanal", boundariesTitle: "Límite humano / automatizable", boundariesHint: "Clasifica cada paso y explica por qué. Esta decisión sigue siendo tuya.", disposition: "Clasificación", rationale: "Racional",
      dispositions: { unassessed: "Sin evaluar", automatable: "Automatizable", "human-required": "Requiere intervención humana" }, safetyTitle: "Seguridad y recuperación", safetyHint: "Declara sensibilidad, efectos y cómo verificar o recuperar excepciones.", sensitivity: "Sensibilidad de datos", sideEffects: "Efectos secundarios", verification: "Modo de verificación", verificationMinutes: "Minutos de verificación por ejecución", recovery: "Plan de recuperación", sensitivities: { unassessed: "Sin evaluar", low: "Baja", moderate: "Moderada", high: "Alta" }, sideEffectLevels: { unassessed: "Sin evaluar", none: "Sin efectos", reversible: "Reversibles", irreversible: "Irreversibles" }, verificationModes: { unassessed: "Sin evaluar", none: "No aplica", sample: "Por muestra", "every-run": "En cada ejecución" },
      simulation: "Simulación local; Rust es la fuente de verdad", estimateTitle: "Estimaciones de planificación", assumptions: "Asumen el mismo tiempo por paso e incluyen la sobrecarga de verificación. Los ahorros no son resultados observados ni garantizados.", baseline: "min/semana manual", residual: "min/semana residual", savings: "min/semana estimados", effort: "esfuerzo", risk: "riesgo operativo", confidence: "confianza",
      localTitle: "Sin conexiones externas", localCopy: "No se conectan servicios de terceros y ningún flujo se ejecuta desde esta pantalla.", save: "Guardar blueprint", saving: "Guardando…", saved: "Blueprint guardado", copy: "Copiar Markdown", copied: "Blueprint copiado", exportTitle: "Copiar o exportar", exportHint: "Disponible solo cuando el blueprint esté listo. Cada acción requiere este gesto explícito.", exportMarkdown: "Exportar Markdown", exportJson: "Exportar JSON", exported: (path) => `Exportado en ${path}`,
      deletePreview: "Ver límite de eliminación", deleteTitle: "Vista previa de eliminación", deleteCopy: "Ensō Time puede borrar sus filas locales y exportaciones administradas. El contenido del portapapeles, copias movidas, backups y copias sincronizadas quedan fuera de ese límite. Esta vista no ejecuta ninguna eliminación.", cancel: "Cancelar", errors: { "blueprint.conflict": "Otra edición cambió este blueprint. Se cargó la versión actual para que la revises.", "blueprint.intake_missing": "No encontramos el levantamiento fuente.", "blueprint.not_found": "No encontramos este blueprint.", "blueprint.not_ready": "Completa el blueprint antes de copiar o exportar.", "blueprint.clipboard": "No pudimos copiar al portapapeles. Revisa los permisos e inténtalo de nuevo.", "blueprint.export": "No pudimos crear el archivo de exportación.", "blueprint.unexpected": "No pudimos completar esta acción. Inténtalo de nuevo." },
    },
    settings: {
      title: "Configuración.", intro: "Decide cuánto registrar, cuándo preguntar y por cuánto tiempo conservarlo.", save: "Guardar cambios", tracking: "Seguimiento", trackingHint: "Frecuencia y horario del estudio.", cadence: "Cadencia", cadenceOption: (minutes) => `Cada ${minutes} minutos`, retention: "Conservar datos", retention30: "30 días", retention90: "90 días", retention365: "1 año", retentionForever: "Sin límite", autostart: "Iniciar con el sistema", autostartHint: "Activa el registro sin abrir la ventana.", titles: "Guardar títulos de ventana", titlesHint: "Mejora el contexto; puede incluir nombres de documentos.", schedule: "Horario de trabajo", scheduleHint: "Los check-ins se mantienen dentro de esta franja.", start: "Inicio", end: "Fin", appearance: "Apariencia e idioma", appearanceHint: "La interfaz cambia; tus datos no.", language: "Idioma", theme: "Tema", ink: "Tinta", paper: "Papel", system: "Sistema", updates: "Buscar nuevas versiones", updatesHint: "Consulta GitHub al abrir la aplicación; no envía actividad.", data: "Datos", localEngine: (port) => port ? `Motor local en 127.0.0.1:${port}.` : "Motor local; sin nube ni telemetría.", exportJson: "Exportar JSON", exportCsv: "Exportar CSV", delete: "Borrar todo", deleteConfirm: "¿Borrar permanentemente toda la información de Ensō Time? Esta acción no se puede deshacer.",
    },
    notices: { settingsSaved: "Configuración guardada", contextSaved: "Contexto guardado", paused: "Registro pausado por 30 minutos", resumed: "Registro reanudado", diagnosticUpdated: "Diagnóstico actualizado", promptCopied: "Prompt copiado", exported: (path) => `Exportado en ${path}`, deleted: "Datos eliminados" },
  },
  en: {
    nav: { today: "Today", checkins: "Check-ins", study: "Study", automate: "Automate", settings: "Settings" },
    common: { local: "Only on this device", loading: "Organizing your time…", retry: "Try again", running: "Tracking", paused: "Paused", degraded: "Incomplete tracking", saved: "Saved", unexpectedError: "We couldn't complete that action. Try again." },
    captureIssue: {
      label: "Tracking needs attention",
      title: "Ensō Time is not capturing everything.",
      retry: "Check again",
      details: {
        "server-unavailable": "The local engine is not responding. Restart Ensō Time and check again; your saved information stays on this device.",
        "window-watcher-unavailable": "We cannot see which app is active. Review Ensō Time's system permissions, then check again.",
        "afk-watcher-unavailable": "We cannot tell when you are away. Keep Ensō Time open and check again to restore that context.",
        "watcher-conflict": "Another session is trying to track this device. Close the duplicate session, then check again.",
      },
    },
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
      title: "What to automate first.", intro: "A recommendation based on what you actually repeat, with visible assumptions.", refresh: "Refresh", preliminaryTitle: "Preliminary reading", preliminaryCopy: (days) => `${days} days have been observed. Use this as a hypothesis, not a verdict.`, candidate: "Top candidate", signal: "signal", weeklyHours: "hrs/week", repetitions: "repetitions", impact: "Impact", feasibility: "Feasibility", risk: "Risk", empty: "There isn't a strong candidate yet.", emptyCopy: "Mark repetitive tasks in your check-ins to build a signal.", recommendations: "Recommendations", promptTitle: "A ready-to-use prompt", promptIntro: "Take the diagnosis with you", copyPrompt: "Copy prompt", workflowAction: "Map workflow", workflowHint: "Document the process before designing an automation.", openChatGpt: "Open ChatGPT", openClaude: "Open Claude", outboundNotice: "Only what you copy leaves the app",
    },
    workflow: {
      context: "Workflow intake",
      title: "Describe how it works today.",
      intro: "Capture the real process, including human judgment and friction. No automation is generated here.",
      candidate: "Candidate",
      back: "Back to diagnosis",
      loading: "Looking for a saved intake…",
      retry: "Try again",
      emptyTitle: "There is no candidate to map.",
      emptyCopy: "Return to the diagnosis and wait for an evidence-backed signal before documenting a workflow.",
      errorTitle: "We couldn't open the intake.",
      loadError: "We couldn't retrieve saved workflow intakes.",
      saveError: "We couldn't save the workflow intake.",
      validation: "Complete the required fields before saving.",
      savedTitle: "Workflow intake saved",
      savedCopy: "You can return, edit it, or continue later. The diagnosis now uses this context.",
      startTitle: "Start and frequency",
      startHint: "Define what starts the process and how much each repetition costs.",
      trigger: "What triggers this workflow?",
      triggerPlaceholder: "E.g. The sales team's weekly close begins",
      frequency: "Frequency",
      frequencyPlaceholder: "E.g. Every Friday",
      minutesPerRun: "Minutes per run",
      toolsTitle: "Tools",
      toolsHint: "Include every app, file, or channel involved.",
      tool: (index) => `Tool ${index}`,
      toolPlaceholder: "E.g. Google Sheets",
      addTool: "Add tool",
      removeTool: (index) => `Remove tool ${index}`,
      stepsTitle: "Ordered steps",
      stepsHint: "Describe what happens, one observable step at a time.",
      step: (index) => `Step ${index}`,
      stepPlaceholder: "E.g. Export the closed opportunities",
      addStep: "Add step",
      removeStep: (index) => `Remove step ${index}`,
      contextTitle: "Judgment and result",
      contextHint: "Separate input data, human decisions, and the expected result.",
      humanDecisions: "Human decisions",
      humanDecisionsPlaceholder: "E.g. Review exceptions and decide which accounts to exclude",
      inputs: "Inputs",
      inputsPlaceholder: "E.g. Sales CSV, target table, and CRM notes",
      desiredOutput: "Desired output",
      desiredOutputPlaceholder: "E.g. A validated report ready to share",
      painPoints: "Pain points",
      painPointsPlaceholder: "E.g. Copying data, fixing formats, and chasing missing fields",
      localTitle: "Saved locally",
      localCopy: "This intake stays in Ensō Time and does not create workflows in external services.",
      save: "Save workflow intake",
      saving: "Saving…",
    },
    blueprint: {
      context: "Automation blueprint", title: "From process to clear boundaries.", intro: "Specify what can be automated and what must remain under human control. We do not connect accounts or run workflows.", back: "Back to diagnosis", loading: "Preparing the blueprint…", retry: "Try again", emptyTitle: "There is no candidate to continue.", emptyCopy: "Return to the diagnosis and choose an evidence-backed recommendation.", mapTitle: "Map the workflow first.", mapCopy: "The blueprint needs a read-only intake of the current process as its source.", mapAction: "Map workflow", errorTitle: "We couldn't open the blueprint.",
      status: "Status", complete: "complete", statuses: { draft: "Draft", ready: "Ready to build", stale: "Source is outdated", blocked: "Blocked" }, staleTitle: "The intake changed.", staleCopy: "Refresh the source before saving, copying, or exporting. Step classifications will reset.", refresh: "Refresh source", refreshed: "Source refreshed; review every step boundary.", blockedTitle: "This blueprint cannot continue yet.", blockedCopy: "Complete the missing information or resolve the source block.",
      evidenceLabel: "Read-only evidence", sourceTitle: "Current process", sourceHint: "Source and observed evidence cannot be edited from this blueprint.", trigger: "Trigger", inputs: "Inputs", output: "Output", tools: "Tools", observed: "Observed", declared: "Declared by you",
      frequencyTitle: "Frequency", frequencyHint: "Only state how many times this happens in a typical week.", runsPerWeek: "Runs per week", boundariesTitle: "Human / automatable boundary", boundariesHint: "Classify every step and explain why. This remains your decision.", disposition: "Disposition", rationale: "Rationale",
      dispositions: { unassessed: "Unassessed", automatable: "Automatable", "human-required": "Human required" }, safetyTitle: "Safety and recovery", safetyHint: "Declare sensitivity, effects, and how to verify or recover exceptions.", sensitivity: "Data sensitivity", sideEffects: "Side effects", verification: "Verification mode", verificationMinutes: "Verification minutes per run", recovery: "Recovery plan", sensitivities: { unassessed: "Unassessed", low: "Low", moderate: "Moderate", high: "High" }, sideEffectLevels: { unassessed: "Unassessed", none: "None", reversible: "Reversible", irreversible: "Irreversible" }, verificationModes: { unassessed: "Unassessed", none: "Not applicable", sample: "Sample", "every-run": "Every run" },
      simulation: "Local simulation; Rust is the source of truth", estimateTitle: "Planning estimates", assumptions: "They assume equal time per step and include verification overhead. Savings are neither observed nor guaranteed outcomes.", baseline: "manual min/week", residual: "residual min/week", savings: "estimated min/week", effort: "effort", risk: "operational risk", confidence: "confidence",
      localTitle: "No external connections", localCopy: "No third-party service is connected and no workflow runs from this screen.", save: "Save blueprint", saving: "Saving…", saved: "Blueprint saved", copy: "Copy Markdown", copied: "Blueprint copied", exportTitle: "Copy or export", exportHint: "Available only when the blueprint is ready. Every action requires this explicit gesture.", exportMarkdown: "Export Markdown", exportJson: "Export JSON", exported: (path) => `Exported to ${path}`,
      deletePreview: "View deletion boundary", deleteTitle: "Deletion preview", deleteCopy: "Ensō Time can delete its local rows and app-managed exports. Clipboard contents, moved exports, backups, and synced copies are outside that boundary. This preview does not run any deletion.", cancel: "Cancel", errors: { "blueprint.conflict": "Another edit changed this blueprint. The current version was loaded for your review.", "blueprint.intake_missing": "We couldn't find the source intake.", "blueprint.not_found": "We couldn't find this blueprint.", "blueprint.not_ready": "Complete the blueprint before copying or exporting.", "blueprint.clipboard": "We couldn't copy to the clipboard. Check permissions and try again.", "blueprint.export": "We couldn't create the export file.", "blueprint.unexpected": "We couldn't complete that action. Try again." },
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
