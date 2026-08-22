---
name: Ensō Time
description: A quiet local instrument for understanding work and choosing what to automate.
colors:
  enso-ink: "#191919"
  graphite-surface: "#242424"
  graphite-raised: "#2D2D2D"
  rice-paper: "#EDEDED"
  quiet-gray: "#AAA9A6"
  signal-blue: "#1F8BFF"
  working-cyan: "#5CD5E8"
  evidence-yellow: "#F8E578"
  action-orange: "#FF5B2E"
  rest-sage: "#8BBFB8"
typography:
  display:
    fontFamily: "Jost, sans-serif"
    fontSize: "clamp(34px, 4vw, 48px)"
    fontWeight: 450
    lineHeight: 1.05
    letterSpacing: "-0.035em"
  title:
    fontFamily: "Jost, sans-serif"
    fontSize: "25px"
    fontWeight: 450
    lineHeight: 1.15
    letterSpacing: "-0.035em"
  body:
    fontFamily: "Inter, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.6
    letterSpacing: "normal"
  label:
    fontFamily: "Inter, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 650
    lineHeight: 1.2
    letterSpacing: "0.11em"
rounded:
  field: "6px"
  control: "7px"
  callout: "9px"
spacing:
  compact: "8px"
  control: "16px"
  section: "24px"
  page: "32px"
components:
  button-primary:
    backgroundColor: "{colors.signal-blue}"
    textColor: "#FFFFFF"
    typography: "{typography.body}"
    rounded: "{rounded.control}"
    padding: "0 15px"
    height: "40px"
  button-secondary:
    backgroundColor: "{colors.graphite-surface}"
    textColor: "{colors.rice-paper}"
    typography: "{typography.body}"
    rounded: "{rounded.control}"
    padding: "0 15px"
    height: "40px"
  field:
    backgroundColor: "{colors.graphite-surface}"
    textColor: "{colors.rice-paper}"
    typography: "{typography.body}"
    rounded: "{rounded.field}"
    padding: "0 12px"
    height: "43px"
---

# Design System: Ensō Time

## 1. Overview

**Creative North Star: "The Quiet Instrument"**

Ensō Time should feel like a precise instrument left open beside the work: observant, legible, and calm enough to disappear until a decision is needed. The composition is flat and editorial, with measured dividers and a single blue signal instead of decorative effects. Ink and rice-paper themes are equal expressions of the same system.

The interface explicitly rejects generic AI productivity dashboards filled with gradients, surveillance software, gamified habit trackers, dense enterprise analytics, and ActivityWatch's full technical dashboard as the primary experience. It favors evidence, compact language, and one useful next move per screen.

**Key Characteristics:**

- Quietly technical, with generous page margins and compact controls.
- Evidence-first hierarchy built from timelines, rules, and thin dividers.
- Restrained color where every accent has semantic meaning.
- Rounded enough to be usable, never soft or toy-like.

## 2. Colors

The palette pairs near-black ink and warm paper with a rare, electric operational blue; supporting colors are reserved for categories and state.

### Primary

- **Signal Blue** (#1F8BFF): Primary actions, active progress, focus rings, and the strongest current-state indicator.

### Secondary

- **Working Cyan** (#5CD5E8): Communication and supportive informational categories.
- **Rest Sage** (#8BBFB8): Healthy idle states, privacy reassurance, and non-urgent success.

### Tertiary

- **Evidence Yellow** (#F8E578): Paused or preliminary states that need attention without alarm.
- **Action Orange** (#FF5B2E): Creation/category emphasis; never a competing global CTA.

### Neutral

- **Ensō Ink** (#191919): Canonical dark canvas and light-theme primary text.
- **Graphite Surface** (#242424): Inputs and quiet grouped surfaces.
- **Graphite Raised** (#2D2D2D): Active navigation and secondary depth.
- **Rice Paper** (#EDEDED): Canonical light canvas and dark-theme primary text.
- **Quiet Gray** (#AAA9A6): Explanations, timestamps, and secondary labels.

### Named Rules

**The One Signal Rule.** Signal Blue should occupy less than roughly ten percent of a screen; its rarity communicates action and live state.

**The Semantic Accent Rule.** Cyan, yellow, orange, and sage identify categories or state. They do not decorate containers.

## 3. Typography

**Display Font:** Jost (with sans-serif fallback)

**Body Font:** Inter (with system-ui fallback)
**Label/Mono Font:** Inter Tight for numeric diagnostic emphasis; Quicksand Medium is reserved for the Ensō wordmark.

**Character:** Jost provides geometric calm and a deliberate editorial voice. Inter remains the official Ensō interface face for dense, multilingual operational copy; its familiarity is intentional, not a generic default.

### Hierarchy

- **Display** (450, clamp(34px, 4vw, 48px), 1.05): Page statements and onboarding only.
- **Headline** (450, 31px, 1.15): Automation candidate and major evidence statements.
- **Title** (450, 25px, 1.15): Section headings.
- **Body** (400, 14px, 1.6): Explanations, prompts, and forms; keep reading lines under 72 characters where possible.
- **Label** (650, 11px, 0.11em, uppercase): Short context markers only, never full-sentence headings.

### Named Rules

**The Statement, Then Evidence Rule.** A Jost statement names the conclusion; compact Inter content immediately shows the evidence behind it.

## 4. Elevation

Ensō Time is flat by default. Depth comes from tonal layering and one-pixel borders, not floating card stacks. The only ambient shadow is reserved for transient UI such as the toast, where separation from changing content is functional.

### Shadow Vocabulary

- **Transient Lift** (`0 18px 48px rgba(0, 0, 0, 0.22)`): Toasts and exceptional overlays only; the paper theme lowers opacity to 0.08.

### Named Rules

**The Flat-at-Rest Rule.** Permanent content uses borders and tonal surfaces. A shadow must represent temporary elevation or interaction.

## 5. Components

### Buttons

- **Shape:** Restrained control radius (7px), 40px minimum height.
- **Primary:** Signal Blue with white text, 15px horizontal padding, and a small 16–17px line icon when useful.
- **Hover / Focus:** Slight brightness change; focus uses a 2px Signal Blue outline with 3px offset.
- **Secondary / Ghost / Tertiary:** Graphite surface plus one-pixel border; no glow, gradient, or oversized pill.

### Chips

- **Style:** Small dot plus text for semantic category; status uses a 7px colored dot and plain label rather than a filled badge.
- **State:** Filled count badges are permitted only for actionable pending check-ins.

### Cards / Containers

- **Corner Style:** Most content is not a card. Callouts may use 9px radius; prompt containers remain nearly rectangular.
- **Background:** Canvas, Graphite Surface, or paper equivalents.
- **Shadow Strategy:** Flat at rest; see Elevation.
- **Border:** One pixel using the neutral border token.
- **Internal Padding:** 24–32px for a true grouped task surface.

### Inputs / Fields

- **Style:** 43px height, 6px radius, one-pixel border, Graphite Surface background.
- **Focus:** 2px Signal Blue outline with 3px offset; meaning is never communicated by color alone.
- **Error / Disabled:** Error copy is explicit; disabled controls reduce opacity but retain readable contrast.

### Navigation

The desktop rail is 224px wide, text-and-icon, and sticky. Active items use Graphite Raised rather than a colored stripe. At medium width it contracts to icons; on mobile it becomes a 64px bottom bar with the same order and pending count.

### Timeline

Each interval has a time, one-pixel vertical rail, semantic dot, application, duration, and category. It is a working record rather than a chart card; empty states replace the record cleanly instead of fabricating sample activity.

## 6. Do's and Don'ts

### Do:

- **Do** use #1F8BFF for one primary action or live signal at a time.
- **Do** preserve one-pixel dividers, 6–9px radii, and compact 40–43px controls.
- **Do** pair every recommendation with visible local evidence.
- **Do** keep both Ink and Rice Paper themes WCAG 2.2 AA and preserve meaning without color.

### Don't:

- **Don't** build generic AI productivity dashboards filled with gradients, glowing cards, and inflated scores.
- **Don't** imitate surveillance software that hides what it records or makes users feel watched.
- **Don't** use gamified habit trackers with streak anxiety, guilt, trophies, or celebratory noise.
- **Don't** turn screens into dense enterprise analytics that require configuration before they become useful.
- **Don't** reproduce ActivityWatch's full technical dashboard as the primary user experience.
- **Don't** use colored side stripes, glassmorphism, gradient text, giant pill controls, or grids of identical rounded cards.
