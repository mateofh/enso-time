import { useEffect, useState, type FormEvent } from "react";
import { AlertCircle, ArrowLeft, Check, Plus, RefreshCw, X } from "lucide-react";
import { bridge } from "../lib/bridge";
import { messages } from "../lib/i18n";
import type { AutomationCandidateV1, Locale, WorkflowIntakeV1 } from "../lib/types";

type LoadState = "loading" | "ready" | "error";

function createEmptyIntake(candidate: AutomationCandidateV1): WorkflowIntakeV1 {
  const now = new Date().toISOString();
  return {
    id: typeof crypto.randomUUID === "function" ? crypto.randomUUID() : `workflow-${candidate.id}-${Date.now()}`,
    candidateId: candidate.id,
    candidateLabel: candidate.activityLabel,
    trigger: "",
    tools: [""],
    steps: [""],
    humanDecisions: "",
    inputs: "",
    desiredOutput: "",
    frequency: "",
    minutesPerRun: 15,
    painPoints: "",
    createdAt: now,
    updatedAt: now,
  };
}

function normalizeIntake(intake: WorkflowIntakeV1, candidate: AutomationCandidateV1): WorkflowIntakeV1 {
  return {
    ...intake,
    candidateId: candidate.id,
    candidateLabel: candidate.activityLabel,
    trigger: intake.trigger.trim(),
    tools: intake.tools.map((item) => item.trim()).filter(Boolean),
    steps: intake.steps.map((item) => item.trim()).filter(Boolean),
    humanDecisions: intake.humanDecisions.trim(),
    inputs: intake.inputs.trim(),
    desiredOutput: intake.desiredOutput.trim(),
    frequency: intake.frequency.trim(),
    minutesPerRun: Math.max(1, Number(intake.minutesPerRun)),
    painPoints: intake.painPoints.trim(),
    updatedAt: new Date().toISOString(),
  };
}

export function WorkflowIntakeView({ candidate, locale, onBack, onSaved }: {
  candidate: AutomationCandidateV1 | null;
  locale: Locale;
  onBack: () => void;
  onSaved: (intake: WorkflowIntakeV1) => Promise<WorkflowIntakeV1>;
}) {
  const copy = messages(locale).workflow;
  const [loadState, setLoadState] = useState<LoadState>(candidate ? "loading" : "ready");
  const [draft, setDraft] = useState<WorkflowIntakeV1 | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string>();
  const [saved, setSaved] = useState(false);
  const [showValidation, setShowValidation] = useState(false);

  async function load() {
    if (!candidate) return;
    setLoadState("loading");
    setSaveError(undefined);
    try {
      const existing = (await bridge.listWorkflowIntakes()).find((item) => item.candidateId === candidate.id);
      setDraft(existing ? { ...existing, tools: existing.tools.length ? existing.tools : [""], steps: existing.steps.length ? existing.steps : [""] } : createEmptyIntake(candidate));
      setLoadState("ready");
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : copy.loadError);
      setLoadState("error");
    }
  }

  useEffect(() => { void load(); }, [candidate?.id]);

  function updateList(key: "tools" | "steps", index: number, value: string) {
    setDraft((current) => current ? { ...current, [key]: current[key].map((item, itemIndex) => itemIndex === index ? value : item) } : current);
  }

  function addListItem(key: "tools" | "steps") {
    setDraft((current) => current ? { ...current, [key]: [...current[key], ""] } : current);
  }

  function removeListItem(key: "tools" | "steps", index: number) {
    setDraft((current) => current ? { ...current, [key]: current[key].filter((_, itemIndex) => itemIndex !== index) } : current);
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!candidate || !draft) return;
    setSaving(true);
    setSaved(false);
    setSaveError(undefined);
    try {
      const savedIntake = await onSaved(normalizeIntake(draft, candidate));
      setDraft(savedIntake);
      setSaved(true);
      setShowValidation(false);
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : copy.saveError);
    } finally {
      setSaving(false);
    }
  }

  if (!candidate) {
    return <section className="workflow-empty"><AlertCircle size={28} aria-hidden="true" /><h1>{copy.emptyTitle}</h1><p>{copy.emptyCopy}</p><button type="button" className="secondary-button" onClick={onBack}><ArrowLeft size={16} aria-hidden="true" />{copy.back}</button></section>;
  }

  if (loadState === "loading") {
    return <section className="workflow-loading" aria-busy="true"><span className="loading-line" /><p>{copy.loading}</p></section>;
  }

  if (loadState === "error" || !draft) {
    return <section className="workflow-empty" role="alert"><AlertCircle size={28} aria-hidden="true" /><h1>{copy.errorTitle}</h1><p>{saveError || copy.loadError}</p><div className="data-actions"><button type="button" className="secondary-button" onClick={onBack}><ArrowLeft size={16} aria-hidden="true" />{copy.back}</button><button type="button" className="primary-button" onClick={() => void load()}><RefreshCw size={16} aria-hidden="true" />{copy.retry}</button></div></section>;
  }

  const set = <K extends keyof WorkflowIntakeV1>(key: K, value: WorkflowIntakeV1[K]) => setDraft((current) => current ? { ...current, [key]: value } : current);

  return (
    <>
      <header className="page-header workflow-header">
        <div><button type="button" className="text-button workflow-back" onClick={onBack}><ArrowLeft size={16} aria-hidden="true" />{copy.back}</button><p className="page-context">{copy.context}</p><h1>{copy.title}</h1><p className="page-intro">{copy.intro}</p></div>
        <div className="workflow-candidate"><span>{copy.candidate}</span><strong>{candidate.activityLabel}</strong></div>
      </header>

      {saved && <div className="workflow-success" role="status"><Check size={17} aria-hidden="true" /><span><strong>{copy.savedTitle}</strong>{copy.savedCopy}</span></div>}
      {saveError && <div className="workflow-error" role="alert"><AlertCircle size={17} aria-hidden="true" /><span><strong>{copy.saveError}</strong>{saveError}</span></div>}
      {showValidation && <p className="workflow-validation" role="alert">{copy.validation}</p>}

      <form className="workflow-form" onSubmit={(event) => void submit(event)} onInvalid={() => setShowValidation(true)}>
        <section className="workflow-section" aria-labelledby="workflow-start-title">
          <div><span>01</span><h2 id="workflow-start-title">{copy.startTitle}</h2><p>{copy.startHint}</p></div>
          <div className="workflow-fields">
            <label>{copy.trigger}<textarea required rows={3} value={draft.trigger} onChange={(event) => set("trigger", event.target.value)} placeholder={copy.triggerPlaceholder} /></label>
            <label>{copy.frequency}<input required value={draft.frequency} onChange={(event) => set("frequency", event.target.value)} placeholder={copy.frequencyPlaceholder} /></label>
            <label>{copy.minutesPerRun}<input required type="number" min={1} inputMode="numeric" value={draft.minutesPerRun} onChange={(event) => set("minutesPerRun", Number(event.target.value))} /></label>
          </div>
        </section>

        <section className="workflow-section" aria-labelledby="workflow-tools-title">
          <div><span>02</span><h2 id="workflow-tools-title">{copy.toolsTitle}</h2><p>{copy.toolsHint}</p></div>
          <div className="workflow-fields dynamic-list">
            {draft.tools.map((tool, index) => <div className="dynamic-row" key={`tool-${index}`}><label>{copy.tool(index + 1)}<input required value={tool} onChange={(event) => updateList("tools", index, event.target.value)} placeholder={copy.toolPlaceholder} /></label>{draft.tools.length > 1 && <button type="button" className="icon-button" aria-label={copy.removeTool(index + 1)} onClick={() => removeListItem("tools", index)}><X size={16} aria-hidden="true" /></button>}</div>)}
            <button type="button" className="text-button add-row" onClick={() => addListItem("tools")}><Plus size={16} aria-hidden="true" />{copy.addTool}</button>
          </div>
        </section>

        <section className="workflow-section" aria-labelledby="workflow-steps-title">
          <div><span>03</span><h2 id="workflow-steps-title">{copy.stepsTitle}</h2><p>{copy.stepsHint}</p></div>
          <ol className="workflow-fields dynamic-list ordered-list">
            {draft.steps.map((step, index) => <li className="dynamic-row" key={`step-${index}`}><label>{copy.step(index + 1)}<textarea required rows={2} value={step} onChange={(event) => updateList("steps", index, event.target.value)} placeholder={copy.stepPlaceholder} /></label>{draft.steps.length > 1 && <button type="button" className="icon-button" aria-label={copy.removeStep(index + 1)} onClick={() => removeListItem("steps", index)}><X size={16} aria-hidden="true" /></button>}</li>)}
            <li className="list-action"><button type="button" className="text-button add-row" onClick={() => addListItem("steps")}><Plus size={16} aria-hidden="true" />{copy.addStep}</button></li>
          </ol>
        </section>

        <section className="workflow-section" aria-labelledby="workflow-context-title">
          <div><span>04</span><h2 id="workflow-context-title">{copy.contextTitle}</h2><p>{copy.contextHint}</p></div>
          <div className="workflow-fields">
            <label>{copy.humanDecisions}<textarea required rows={3} value={draft.humanDecisions} onChange={(event) => set("humanDecisions", event.target.value)} placeholder={copy.humanDecisionsPlaceholder} /></label>
            <label>{copy.inputs}<textarea required rows={3} value={draft.inputs} onChange={(event) => set("inputs", event.target.value)} placeholder={copy.inputsPlaceholder} /></label>
            <label>{copy.desiredOutput}<textarea required rows={3} value={draft.desiredOutput} onChange={(event) => set("desiredOutput", event.target.value)} placeholder={copy.desiredOutputPlaceholder} /></label>
            <label>{copy.painPoints}<textarea required rows={3} value={draft.painPoints} onChange={(event) => set("painPoints", event.target.value)} placeholder={copy.painPointsPlaceholder} /></label>
          </div>
        </section>

        <div className="workflow-actions"><p><strong>{copy.localTitle}</strong>{copy.localCopy}</p><button type="submit" className="primary-button" disabled={saving}><Check size={17} aria-hidden="true" />{saving ? copy.saving : copy.save}</button></div>
      </form>
    </>
  );
}
