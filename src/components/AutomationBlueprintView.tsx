import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { AlertCircle, ArrowLeft, Check, Clipboard, Download, RefreshCw, ShieldCheck } from "lucide-react";
import { BlueprintBridgeError, bridge } from "../lib/bridge";
import { messages } from "../lib/i18n";
import type { AutomationBlueprintDraftV1, AutomationBlueprintV1, AutomationCandidateV1, Locale, StepDispositionV1 } from "../lib/types";

type LoadState = "loading" | "ready" | "error";

function errorCode(error: unknown) {
  return error instanceof BlueprintBridgeError ? error.code : error instanceof Error ? error.message : "blueprint.unexpected";
}

export function AutomationBlueprintView({ candidate, locale, onBack, onMapWorkflow, onDeleted }: {
  candidate: AutomationCandidateV1 | null;
  locale: Locale;
  onBack: () => void;
  onMapWorkflow: (candidate: AutomationCandidateV1) => void;
  onDeleted: () => void;
}) {
  const copy = messages(locale).blueprint;
  const title = useRef<HTMLHeadingElement>(null);
  const [loadState, setLoadState] = useState<LoadState>(candidate ? "loading" : "ready");
  const [blueprint, setBlueprint] = useState<AutomationBlueprintV1 | null>(null);
  const [draft, setDraft] = useState<AutomationBlueprintDraftV1 | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [showDeletePreview, setShowDeletePreview] = useState(false);
  const deleteActivator = useRef<HTMLButtonElement>(null);
  const deleteDialog = useRef<HTMLElement>(null);
  const deleteCancel = useRef<HTMLButtonElement>(null);

  async function load() {
    if (!candidate) return;
    setLoadState("loading"); setError(undefined);
    try {
      const next = await bridge.getAutomationBlueprint(candidate.id, locale);
      setBlueprint(next); setDraft(next?.draft ?? null); setLoadState("ready");
      window.setTimeout(() => title.current?.focus(), 0);
    } catch (cause) {
      setError(errorCode(cause)); setLoadState("error");
    }
  }

  useEffect(() => { void load(); }, [candidate?.id, locale]);
  useEffect(() => {
    if (showDeletePreview) deleteCancel.current?.focus();
  }, [showDeletePreview]);

  function closeDeletePreview() {
    setShowDeletePreview(false);
    window.setTimeout(() => deleteActivator.current?.focus(), 0);
  }

  function trapDeleteDialog(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeDeletePreview();
      return;
    }
    if (event.key !== "Tab") return;
    const focusable = [...(deleteDialog.current?.querySelectorAll<HTMLElement>('button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])') ?? [])];
    if (!focusable.length) return;
    const first = focusable[0]!;
    const last = focusable.at(-1)!;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault(); last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault(); first.focus();
    }
  }

  function update<K extends keyof AutomationBlueprintDraftV1>(key: K, value: AutomationBlueprintDraftV1[K]) {
    setDraft((current) => current ? { ...current, [key]: value } : current);
  }

  function updateBoundary(stepId: string, disposition: StepDispositionV1, rationale?: string) {
    setDraft((current) => current ? {
      ...current,
      stepBoundaries: current.stepBoundaries.map((item) => item.stepId === stepId ? { ...item, disposition, rationale: disposition === "unassessed" ? undefined : rationale ?? item.rationale } : item),
    } : current);
  }

  async function save() {
    if (!draft || !blueprint || blueprint.status === "stale") return;
    setBusy(true); setError(undefined); setNotice(undefined);
    try {
      const next = await bridge.saveAutomationBlueprint({ draft, expectedRevision: blueprint.draft.revision }, locale);
      setBlueprint(next); setDraft(next.draft); setNotice(copy.saved);
    } catch (cause) {
      const code = errorCode(cause);
      if (cause instanceof BlueprintBridgeError && cause.code === "blueprint.conflict" && cause.current) {
        setBlueprint(cause.current); setDraft(cause.current.draft);
      }
      setError(code);
    } finally { setBusy(false); }
  }

  async function refresh() {
    if (!blueprint || !candidate) return;
    setBusy(true); setError(undefined);
    try {
      const next = await bridge.refreshAutomationBlueprintSource(candidate.id, blueprint.draft.revision, locale);
      setBlueprint(next); setDraft(next.draft); setNotice(copy.refreshed);
    } catch (cause) { setError(errorCode(cause)); } finally { setBusy(false); }
  }

  async function copyBlueprint() {
    if (!blueprint || blueprint.status !== "ready") return;
    setBusy(true); setError(undefined);
    try { await navigator.clipboard.writeText(blueprint.renderedMarkdown); setNotice(copy.copied); }
    catch { setError("blueprint.clipboard"); } finally { setBusy(false); }
  }

  async function exportBlueprint(format: "markdown" | "json") {
    if (!blueprint || !candidate || blueprint.status !== "ready") return;
    setBusy(true); setError(undefined);
    try { const path = await bridge.exportAutomationBlueprint(candidate.id, blueprint.draft.revision, format, locale); setNotice(copy.exported(path)); }
    catch (cause) { setError(errorCode(cause) === "blueprint.unexpected" ? "blueprint.export" : errorCode(cause)); } finally { setBusy(false); }
  }

  async function deleteBlueprint() {
    if (!blueprint || !candidate) return;
    setBusy(true); setError(undefined);
    try {
      await bridge.deleteAutomationBlueprint(candidate.id, blueprint.draft.revision, "DELETE_AUTOMATION_BLUEPRINT", locale);
      closeDeletePreview();
      onDeleted();
    } catch (cause) {
      const code = errorCode(cause);
      setError(code === "blueprint.unexpected" ? "blueprint.deleteError" : code);
      if (code === "blueprint.conflict") {
        await load();
        setError("blueprint.conflict");
      }
    } finally { setBusy(false); }
  }

  if (!candidate) return <section className="workflow-empty"><AlertCircle size={28} aria-hidden="true" /><h1>{copy.emptyTitle}</h1><p>{copy.emptyCopy}</p><button type="button" className="secondary-button" onClick={onBack}><ArrowLeft size={16} aria-hidden="true" />{copy.back}</button></section>;
  if (loadState === "loading") return <section className="workflow-loading" data-testid="blueprint-loading" aria-busy="true"><span className="loading-line" /><p>{copy.loading}</p></section>;
  if (loadState === "error") return <section className="workflow-empty" data-testid="blueprint-error" role="alert"><AlertCircle size={28} aria-hidden="true" /><h1>{copy.errorTitle}</h1><p>{copy.errors[error ?? ""] ?? error ?? copy.errors["blueprint.unexpected"]}</p><button type="button" className="primary-button" data-testid="blueprint-retry" onClick={() => void load()}><RefreshCw size={16} aria-hidden="true" />{copy.retry}</button></section>;
  if (!blueprint || !draft) return <section className="workflow-empty"><ShieldCheck size={28} aria-hidden="true" /><h1>{copy.mapTitle}</h1><p>{copy.mapCopy}</p><button type="button" className="primary-button" onClick={() => onMapWorkflow(candidate)}>{copy.mapAction}</button></section>;

  const isReady = blueprint.status === "ready";
  const boundaries = new Map(draft.stepBoundaries.map((item) => [item.stepId, item]));
  return <section className="blueprint-panel" data-testid="blueprint-panel">
    <header className="page-header workflow-header blueprint-header">
      <div><button type="button" className="text-button workflow-back" onClick={onBack}><ArrowLeft size={16} aria-hidden="true" />{copy.back}</button><p className="page-context">{copy.context}</p><h1 ref={title} tabIndex={-1}>{copy.title}</h1><p className="page-intro">{copy.intro}</p></div>
      <div className={`blueprint-status is-${blueprint.status}`} data-testid="blueprint-status"><span>{copy.status}</span><strong>{copy.statuses[blueprint.status]}</strong><small data-testid="blueprint-completeness">{blueprint.completeness.score}/100 {copy.complete}</small></div>
    </header>

    {error && <div className="workflow-error" role="alert"><AlertCircle size={17} aria-hidden="true" /><span><strong>{copy.errorTitle}</strong>{copy.errors[error] ?? error ?? copy.errors["blueprint.unexpected"]}</span></div>}
    {notice && <div className="workflow-success" data-testid={notice === copy.refreshed ? "blueprint-refresh-confirmation" : undefined} role="status"><Check size={17} aria-hidden="true" /><span>{notice}</span></div>}
    {blueprint.conflict && <div className="workflow-error" data-testid="blueprint-conflict" role="alert"><AlertCircle size={17} aria-hidden="true" /><span><strong>{copy.errors["blueprint.conflict"]}</strong>{blueprint.conflict.currentRevision}</span></div>}
    {blueprint.status === "stale" && <section className="blueprint-alert" role="alert"><AlertCircle size={18} aria-hidden="true" /><div><strong>{copy.staleTitle}</strong><p>{copy.staleCopy}</p></div><button type="button" className="primary-button" data-testid="blueprint-refresh" disabled={busy} onClick={() => void refresh()}><RefreshCw size={16} aria-hidden="true" />{copy.refresh}</button></section>}
    {blueprint.status === "blocked" && <section className="blueprint-alert" role="alert"><AlertCircle size={18} aria-hidden="true" /><div><strong>{copy.blockedTitle}</strong><p>{copy.blockedCopy}</p></div></section>}

    <section className="blueprint-evidence" aria-labelledby="evidence-title">
      <div><p className="section-note">{copy.evidenceLabel}</p><h2 id="evidence-title">{copy.sourceTitle}</h2><p>{copy.sourceHint}</p></div>
      <div className="source-map" data-testid="blueprint-source-map"><dl><dt>{copy.trigger}</dt><dd>{blueprint.source.trigger}</dd><dt>{copy.inputs}</dt><dd>{blueprint.source.inputs}</dd><dt>{copy.output}</dt><dd>{blueprint.source.desiredOutput}</dd><dt>{copy.tools}</dt><dd>{blueprint.source.tools.join(" · ")}</dd><dt>{copy.declared}</dt><dd>{blueprint.source.humanDecisions}</dd></dl><div className="evidence-columns"><div><strong>{copy.observed}</strong><ul>{blueprint.evidence.observed.map((item) => <li key={item}>{item}</li>)}</ul><small data-testid="blueprint-evidence-count">{blueprint.evidence.repeatedObservations}</small></div><div><strong>{copy.declared}</strong><ul>{blueprint.evidence.declared.map((item) => <li key={item}>{item}</li>)}</ul></div></div></div>
    </section>

    <form className="blueprint-form" onSubmit={(event) => { event.preventDefault(); void save(); }}>
      <section className="workflow-section" aria-labelledby="blueprint-frequency-title"><div><span>01</span><h2 id="blueprint-frequency-title">{copy.frequencyTitle}</h2><p>{copy.frequencyHint}</p></div><div className="workflow-fields"><label>{copy.runsPerWeek}<input required type="number" min={1} max={1000} inputMode="numeric" value={draft.runsPerWeek ?? ""} onChange={(event) => update("runsPerWeek", event.target.value ? Number(event.target.value) : undefined)} /></label></div></section>
      <section className="workflow-section" aria-labelledby="blueprint-boundaries-title"><div><span>02</span><h2 id="blueprint-boundaries-title">{copy.boundariesTitle}</h2><p>{copy.boundariesHint}</p></div><div className="workflow-fields">{blueprint.source.steps.map((step, index) => { const boundary = boundaries.get(step.id); return <fieldset className="boundary-row" data-testid="blueprint-boundary" key={step.id}><legend>{index + 1}. {step.label}</legend><label>{copy.disposition}<select value={boundary?.disposition ?? "unassessed"} onChange={(event) => updateBoundary(step.id, event.target.value as StepDispositionV1)}><option value="unassessed">{copy.dispositions.unassessed}</option><option value="automatable">{copy.dispositions.automatable}</option><option value="human-required">{copy.dispositions["human-required"]}</option></select></label>{boundary?.disposition !== "unassessed" && <label>{copy.rationale}<textarea required rows={2} value={boundary?.rationale ?? ""} onChange={(event) => updateBoundary(step.id, boundary!.disposition, event.target.value)} /></label>}</fieldset>; })}</div></section>
      <section className="workflow-section" aria-labelledby="blueprint-safety-title"><div><span>03</span><h2 id="blueprint-safety-title">{copy.safetyTitle}</h2><p>{copy.safetyHint}</p></div><div className="workflow-fields"><label>{copy.sensitivity}<select value={draft.dataSensitivity} onChange={(event) => update("dataSensitivity", event.target.value as AutomationBlueprintDraftV1["dataSensitivity"])}>{(["unassessed", "low", "moderate", "high"] as const).map((item) => <option value={item} key={item}>{copy.sensitivities[item]}</option>)}</select></label><label>{copy.sideEffects}<select value={draft.sideEffectLevel} onChange={(event) => update("sideEffectLevel", event.target.value as AutomationBlueprintDraftV1["sideEffectLevel"])}>{(["unassessed", "none", "reversible", "irreversible"] as const).map((item) => <option value={item} key={item}>{copy.sideEffectLevels[item]}</option>)}</select></label><label>{copy.verification}<select value={draft.verificationMode} onChange={(event) => update("verificationMode", event.target.value as AutomationBlueprintDraftV1["verificationMode"])}>{(["unassessed", "none", "sample", "every-run"] as const).map((item) => <option value={item} key={item}>{copy.verificationModes[item]}</option>)}</select></label><label>{copy.verificationMinutes}<input required={draft.verificationMode !== "unassessed" && draft.verificationMode !== "none"} type="number" min={0} max={1440} value={draft.verificationMinutesPerRun ?? ""} onChange={(event) => update("verificationMinutesPerRun", event.target.value ? Number(event.target.value) : undefined)} /></label><label>{copy.recovery}<textarea required rows={3} value={draft.recoveryPlan ?? ""} onChange={(event) => update("recoveryPlan", event.target.value)} /></label></div></section>
      <section className="blueprint-estimates" aria-labelledby="estimate-title"><div><p className="section-note">{copy.simulation}</p><h2 id="estimate-title">{copy.estimateTitle}</h2><p data-testid="blueprint-assumptions">{copy.assumptions}</p></div><div className="estimate-grid"><span><b>{blueprint.estimates.weeklyBaselineMinutes}</b>{copy.baseline}</span><span><b>{blueprint.estimates.weeklyResidualMinutes}</b>{copy.residual}</span><span><b>{blueprint.estimates.weeklySavingsMinutes}</b>{copy.savings}</span><span><b>{blueprint.estimates.effort}/100</b>{copy.effort}</span><span><b>{blueprint.estimates.operationalRisk}/100</b>{copy.risk}</span><span><b>{blueprint.estimates.confidence}/100</b>{copy.confidence}</span></div></section>
      <div className="workflow-actions"><p><strong>{copy.localTitle}</strong>{copy.localCopy}</p><button type="submit" className="primary-button" data-testid="blueprint-save" disabled={busy || Boolean(blueprint.conflict) || blueprint.status === "stale" || blueprint.status === "blocked"}><Check size={17} aria-hidden="true" />{busy ? copy.saving : copy.save}</button></div>
    </form>
    <section className="blueprint-export" aria-labelledby="export-title"><div><h2 id="export-title">{copy.exportTitle}</h2><p>{copy.exportHint}</p></div><div className="data-actions"><button type="button" className="secondary-button" data-testid="blueprint-copy" disabled={!isReady || busy} onClick={() => void copyBlueprint()}><Clipboard size={16} aria-hidden="true" />{copy.copy}</button><button type="button" className="secondary-button" data-testid="blueprint-export" disabled={!isReady || busy} onClick={() => void exportBlueprint("markdown")}><Download size={16} aria-hidden="true" />{copy.exportMarkdown}</button><button type="button" className="secondary-button" disabled={!isReady || busy} onClick={() => void exportBlueprint("json")}><Download size={16} aria-hidden="true" />{copy.exportJson}</button><button type="button" className="text-button" ref={deleteActivator} data-testid="blueprint-delete-preview" onClick={() => setShowDeletePreview(true)}>{copy.deletePreview}</button></div></section>
    {showDeletePreview && <div className="blueprint-dialog-backdrop"><section className="blueprint-dialog" ref={deleteDialog} role="dialog" aria-modal="true" aria-labelledby="blueprint-delete-title" aria-describedby="blueprint-delete-copy" onKeyDown={trapDeleteDialog}><h2 id="blueprint-delete-title">{copy.deleteTitle}</h2><p id="blueprint-delete-copy">{copy.deleteCopy}</p>{error && <p className="blueprint-dialog-error" role="alert">{copy.errors[error] ?? copy.deleteError}</p>}<div className="data-actions"><button type="button" className="secondary-button" ref={deleteCancel} disabled={busy} onClick={closeDeletePreview}>{copy.cancel}</button><button type="button" className="danger-button" disabled={busy} onClick={() => void deleteBlueprint()}>{copy.confirmDelete}</button></div></section></div>}
  </section>;
}
