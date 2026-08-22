# Ensō Time roadmap

This roadmap records product intent, not a promise of release dates. Each stage must
remain local-first, optional, and testable without sending a user's activity data to a
third party.

## v0.1 public beta — Decide where to start

Goal: help someone who does not know where to apply AI find a defensible first
opportunity from their real work.

- Observe active applications and AFK periods locally.
- Ask for context only when the captured evidence is insufficient.
- Identify repeated work, context switching, and likely bottlenecks.
- Recommend what to automate first and explain the evidence, feasibility, and risk.
- Guide the user through a local, editable workflow intake for the top candidate.
- Produce a copyable diagnostic prompt; never send it automatically.

The beta does not connect accounts, execute automations, or change external systems.

## v0.2 — Automation Blueprint

Goal: turn a completed workflow intake into a build-ready specification while keeping
the user in control.

### Scope

1. Score intake completeness and ask only for missing evidence.
2. Render the current process as trigger, inputs, ordered steps, decisions, tools, and
   output.
3. Separate human judgment from deterministic steps that may be automated.
4. Estimate time saved, implementation effort, operational risk, and confidence.
5. Generate a vendor-neutral automation blueprint and a copyable prompt suitable for
   starting implementation in n8n, Make, Zapier, or custom code.
6. Keep every draft local and require an explicit copy/export action.

### Out of scope

- Connecting third-party accounts or collecting OAuth credentials.
- Creating, enabling, or changing a workflow in an external platform.
- Claiming guaranteed savings or presenting weak evidence as fact.
- Choosing a vendor without showing the assumptions and tradeoffs.

### Definition of done

A user can move from “I do not know where to start with AI” to one prioritized,
evidence-backed automation with a complete current-state map, human/automation
boundary, risk assessment, and build-ready blueprint. Desktop and mobile-width UI,
local persistence, export, deletion, bilingual copy, and failure states are covered by
automated tests.

## Later — Calendar context

Calendar context follows Automation Blueprint. Start with optional read-only iCalendar
import, then consider Google Calendar and Microsoft 365 behind separate consent and
least-privilege OAuth scopes. Its purpose is to compare planned time with observed time,
not to create, move, or delete events automatically.

## Release gates for every stage

- No silent success: degraded capture or generation states must be visible.
- Local data boundaries, export behavior, and deletion limits stay documented.
- Cross-platform CI and installer smoke tests pass before tagging a release.
- Security and code review have no unresolved critical, high, or medium findings.
- Public claims are supported by product evidence or identified as hypotheses.
