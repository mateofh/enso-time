import type { CategoryId, Locale } from "./types";

const copy = {
  "es-419": {
    nav: { today: "Hoy", checkins: "Check-ins", study: "Estudio", automate: "Automatizar", settings: "Configuración" },
    categories: {
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
    common: { save: "Guardar", cancel: "Cancelar", close: "Cerrar", local: "Todo permanece local", loading: "Organizando tu tiempo…" },
  },
  en: {
    nav: { today: "Today", checkins: "Check-ins", study: "Study", automate: "Automate", settings: "Settings" },
    categories: {
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
    common: { save: "Save", cancel: "Cancel", close: "Close", local: "Everything stays local", loading: "Organizing your time…" },
  },
} as const;

export function messages(locale: Locale) {
  return copy[locale];
}

export function categoryLabel(locale: Locale, category: CategoryId): string {
  return copy[locale].categories[category];
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

