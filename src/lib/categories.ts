import type { CategoryId } from "./types";

export const CATEGORY_ORDER: CategoryId[] = [
  "focus",
  "communication",
  "meetings",
  "admin",
  "learning",
  "creation",
  "personal",
  "break",
  "unknown",
];

export const CATEGORY_COLORS: Record<CategoryId, string> = {
  focus: "var(--blue)",
  communication: "var(--cyan)",
  meetings: "var(--yellow)",
  admin: "var(--sage)",
  learning: "var(--blue-alt)",
  creation: "var(--orange)",
  personal: "var(--ink-subtle)",
  break: "var(--surface-raised)",
  unknown: "var(--muted)",
};

export function categoryColor(category: CategoryId): string {
  return CATEGORY_COLORS[category];
}

