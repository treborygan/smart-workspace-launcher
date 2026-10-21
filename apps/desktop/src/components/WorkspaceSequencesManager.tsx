import { useEffect, useState } from "react";
import type { WorkspaceSequence, WorkspaceSequenceRunResult, WorkspaceSequenceStep } from "@sw/contracts";
import type { Resource } from "../lib/contracts";

const MAX_STEPS = 20;
type Props = {
  profileId: string;
  sequences: WorkspaceSequence[];
  resources: Resource[];
  onClose: () => void;
  onSave: (sequence: WorkspaceSequence) => Promise<WorkspaceSequence>;
  onDelete: (id: string) => Promise<void>;
  onRun: (id: string) => Promise<WorkspaceSequenceRunResult>;
};

const blank = (profileId: string): WorkspaceSequence => ({ id: crypto.randomUUID(), profileId, name: "", steps: [] });

export function WorkspaceSequencesManager({ profileId, sequences, resources, onClose, onSave, onDelete, onRun }: Props) {
  const [draft, setDraft] = useState<WorkspaceSequence>(() => blank(profileId));
  const [runningId, setRunningId] = useState<string | null>(null);
  const [runResult, setRunResult] = useState<WorkspaceSequenceRunResult | null>(null);
  const [savedIds, setSavedIds] = useState<string[]>([]);
  const [savedVersion, setSavedVersion] = useState<WorkspaceSequence | null>(null);
  const [error, setError] = useState("");
  useEffect(() => { setDraft(blank(profileId)); setSavedVersion(null); setRunResult(null); setError(""); }, [profileId]);
  const setStep = (index: number, patch: Partial<WorkspaceSequenceStep>) => setDraft((current) => ({ ...current, steps: current.steps.map((step, at) => at === index ? { ...step, ...patch } : step) }));
  const addStep = () => {
    const first = resources.find((resource) => resource.targets.some((target) => target.active));
    if (!first || draft.steps.length >= MAX_STEPS) return;
    const target = first.targets.find((item) => item.active);
    setDraft((current) => ({ ...current, steps: [...current.steps, { id: crypto.randomUUID(), resourceId: first.id, ...(target ? { targetId: target.id } : {}) }] }));
  };
  const save = async () => {
    setError("");
    if (!draft.name.trim()) { setError("Enter a name for this sequence."); return; }
    if (draft.steps.length < 1 || draft.steps.length > MAX_STEPS) { setError(`Add 1 to ${MAX_STEPS} resource steps.`); return; }
    try { const saved = await onSave({ ...draft, name: draft.name.trim() }); setDraft(saved); setSavedVersion(saved); setSavedIds((current) => current.includes(saved.id) ? current : [...current, saved.id]); setRunResult(null); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Could not save this sequence."); }
  };
  const run = async (sequence: WorkspaceSequence) => {
    setError(""); setRunResult(null); setRunningId(sequence.id);
    try { setRunResult(await onRun(sequence.id)); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Could not run this sequence."); }
    finally { setRunningId(null); }
  };
  const deleteSequence = async (id: string) => {
    setError("");
    try { await onDelete(id); if (draft.id === id) { setDraft(blank(profileId)); setSavedVersion(null); setRunResult(null); } }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Could not delete this sequence."); }
  };
  const move = (index: number, delta: number) => setDraft((current) => {
    const next = [...current.steps]; const destination = index + delta;
    if (destination < 0 || destination >= next.length) return current;
    [next[index], next[destination]] = [next[destination], next[index]];
    return { ...current, steps: next };
  });
  const title = sequences.find((item) => item.id === draft.id)?.name || (savedIds.includes(draft.id) ? draft.name : undefined);
  const currentSaved = sequences.find((item) => item.id === draft.id) || savedVersion;
  const isDirty = Boolean(currentSaved && (draft.name !== currentSaved.name || JSON.stringify(draft.steps) !== JSON.stringify(currentSaved.steps)));
  return <section className="workspace-sequences-manager">
    <div className="welcome-row"><div><h1>Workspace sequences</h1><p>Run a saved, ordered set of workspace resources. Each step uses the normal launch checks and stops after the first failed handoff.</p></div><button className="button secondary" onClick={onClose}>Back to workspace</button></div>
    <div className="workspace-sequences-layout">
      <section className="workspace-sequence-list" aria-label="Saved sequences">
        <header><h2>Saved sequences</h2><button className="button primary" onClick={() => { setDraft(blank(profileId)); setSavedVersion(null); setRunResult(null); setError(""); }}>New sequence</button></header>
        {sequences.map((sequence) => <article className="workspace-sequence-row" key={sequence.id}>
          <button className="workspace-sequence-edit" onClick={() => { setDraft(sequence); setSavedVersion(sequence); setRunResult(null); setError(""); }}><b>{sequence.name}</b><small>{sequence.steps.length} {sequence.steps.length === 1 ? "step" : "steps"}</small></button>
          <button className="button secondary" disabled={runningId !== null} onClick={() => void run(sequence)}>{runningId === sequence.id ? "Running…" : "Run sequence"}</button>
          <button className="icon-button" aria-label={`Delete ${sequence.name}`} disabled={runningId !== null} onClick={() => void deleteSequence(sequence.id)}>×</button>
        </article>)}
        {sequences.length === 0 && <p className="empty-state">No sequences yet. Create one by choosing resources in order.</p>}
      </section>
      <section className="workspace-sequence-form" aria-label="Sequence editor">
        <h2>{title ? `Edit ${title}` : "Create a sequence"}</h2>
        <label>Sequence name<input aria-label="Sequence name" maxLength={100} value={draft.name} onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))} placeholder="e.g. Start my planning session" /></label>
        <div className="workspace-sequence-steps"><h3>Ordered steps <small>{draft.steps.length}/{MAX_STEPS}</small></h3>
          {draft.steps.map((step, index) => {
            const resource = resources.find((item) => item.id === step.resourceId);
            const targets = resource?.targets.filter((item) => item.active) || [];
            return <div className="workspace-sequence-step" key={step.id}>
              <span className="workspace-sequence-number">{index + 1}</span>
              <select aria-label={`Resource for step ${index + 1}`} value={step.resourceId} onChange={(event) => { const chosen = resources.find((item) => item.id === event.target.value); const target = chosen?.targets.find((item) => item.active); setStep(index, { resourceId: event.target.value, targetId: target?.id }); }}>
                {resources.map((item) => <option key={item.id} value={item.id}>{item.displayName}</option>)}
              </select>
              {targets.length > 1 && <select aria-label={`Target for step ${index + 1}`} value={step.targetId || ""} onChange={(event) => setStep(index, { targetId: event.target.value || undefined })}><option value="">Default active target</option>{targets.map((target) => <option key={target.id} value={target.id}>{target.canonicalUri}</option>)}</select>}
              <button className="move-card" aria-label={`Move step ${index + 1} earlier`} disabled={index === 0} onClick={() => move(index, -1)}>↑</button>
              <button className="move-card" aria-label={`Move step ${index + 1} later`} disabled={index === draft.steps.length - 1} onClick={() => move(index, 1)}>↓</button>
              <button className="icon-button" aria-label={`Remove step ${index + 1}`} onClick={() => setDraft((current) => ({ ...current, steps: current.steps.filter((_, at) => at !== index) }))}>×</button>
            </div>;
          })}
          <button className="button secondary" disabled={draft.steps.length >= MAX_STEPS || !resources.some((resource) => resource.targets.some((target) => target.active))} onClick={addStep}>Add resource step</button>
        </div>
        {error && <p role="alert" className="form-error">{error}</p>}
        <p className="workspace-sequence-note">Sequences contain resource references only. They do not run scripts or commands. Imported sequences stay inactive until you choose Run sequence.</p>
        <footer><button className="button primary" onClick={() => void save()}>Save sequence</button>{title && <button className="button secondary" disabled={runningId !== null || isDirty} onClick={() => void run(draft)}>{runningId === draft.id ? "Running…" : "Run sequence"}</button>}</footer>
        {runResult && <div className={`workspace-sequence-result ${runResult.status}`} role="status"><b>{runResult.status === "completed" ? "Sequence completed" : "Sequence stopped"}</b><span>{runResult.completedSteps} of {runResult.totalSteps} steps completed.</span>{runResult.stoppedAtStepId && <span>Stopped at step {runResult.steps.findIndex((item) => item.stepId === runResult.stoppedAtStepId) + 1}. {runResult.steps.at(-1)?.result.message || "The resource could not be handed off."}</span>}<ol>{runResult.steps.map((item) => <li key={item.stepId}>{resources.find((resource) => resource.id === item.resourceId)?.displayName || "Resource"}: {item.result.status === "failed" ? "failed" : "handed off"}</li>)}</ol></div>}
      </section>
    </div>
  </section>;
}
