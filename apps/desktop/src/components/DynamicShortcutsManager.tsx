import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import type { DynamicShortcut } from "@sw/contracts";
import type { Resource } from "../lib/contracts";

type Draft = { name: string; resourceId: string; targetId: string };
const blank: Draft = { name: "", resourceId: "", targetId: "" };
export function DynamicShortcutsManager({ profileId, resources, shortcuts, onSave, onDelete, onClose }: {
  profileId: string;
  resources: Resource[];
  shortcuts: DynamicShortcut[];
  onSave: (shortcut: DynamicShortcut) => Promise<DynamicShortcut>;
  onDelete: (id: string) => Promise<void>;
  onClose: () => void;
}) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft>(blank);
  const [error, setError] = useState(""); const [status, setStatus] = useState(""); const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (!editingId) return;
    const current = shortcuts.find((shortcut) => shortcut.id === editingId);
    if (current) setDraft({ name: current.name, resourceId: current.resourceId, targetId: current.targetId || "" });
  }, [editingId, shortcuts]);
  const selectedResource = resources.find((resource) => resource.id === draft.resourceId);
  const eligibleTargets = selectedResource?.targets.filter((target) => target.active) || [];
  const save = async (event: FormEvent) => {
    event.preventDefault(); setError(""); setStatus("");
    const name = draft.name.trim();
    if (!name) { setError("Shortcut name is required."); return; }
    if ([...name].length > 100) { setError("Shortcut name must be 100 characters or fewer."); return; }
    if (/[\u0000-\u001f\u007f-\u009f]/.test(name)) { setError("Shortcut name cannot contain control characters."); return; }
    if (!selectedResource) { setError("Choose a resource in this profile."); return; }
    if (draft.targetId && !eligibleTargets.some((target) => target.id === draft.targetId)) { setError("Choose an active target owned by the selected resource."); return; }
    setBusy(true);
    try {
      const saved = await onSave({ id: editingId || crypto.randomUUID(), profileId, name, resourceId: selectedResource.id, ...(draft.targetId ? { targetId: draft.targetId } : {}) });
      setEditingId(null); setDraft(blank); setStatus("Quick Launch shortcut saved.");
      return saved;
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Could not save this shortcut."); }
    finally { setBusy(false); }
  };
  const remove = async (shortcut: DynamicShortcut) => {
    if (!window.confirm(`Delete “${shortcut.name}” from Quick Launch? The resource will remain unchanged.`)) return;
    setBusy(true); setError("");
    try { await onDelete(shortcut.id); if (editingId === shortcut.id) { setEditingId(null); setDraft(blank); } setStatus(`${shortcut.name} deleted.`); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Could not delete this shortcut."); }
    finally { setBusy(false); }
  };
  const startNew = () => { setEditingId(null); setDraft({ ...blank, resourceId: resources[0]?.id || "" }); setError(""); setStatus(""); };
  return <section className="dynamic-shortcuts-manager" aria-label="Quick Launch shortcuts manager">
    <div className="welcome-row"><div><h1>Quick Launch shortcuts</h1><p>Save a name that opens an existing workspace resource. Shortcuts cannot run arbitrary commands.</p></div><button className="button secondary" onClick={onClose}>Done</button></div>
    <div className="smart-collections-layout"><section className="smart-collection-list" aria-label="Saved Quick Launch shortcuts"><div className="smart-collection-list-head"><h2>Saved shortcuts</h2><button className="button primary" onClick={startNew} disabled={!resources.length}>New shortcut</button></div>{shortcuts.length ? shortcuts.map((shortcut) => { const resource = resources.find((item) => item.id === shortcut.resourceId); const target = resource?.targets.find((item) => item.id === shortcut.targetId); return <article className="smart-collection-row" key={shortcut.id}><button className="smart-collection-edit" onClick={() => { setError(""); setStatus(""); setEditingId(shortcut.id); }}><b>{shortcut.name}</b><small>{resource?.displayName || "Resource unavailable"}{target ? ` · ${target.canonicalUri}` : " · default target"}</small></button><button className="text-button danger" disabled={busy} onClick={() => void remove(shortcut)}>Delete</button></article>; }) : <p className="empty-state">No saved Quick Launch shortcuts yet.</p>}</section>
      <form className="smart-collection-form" aria-label={editingId ? "Edit Quick Launch shortcut" : "New Quick Launch shortcut"} onSubmit={(event) => void save(event)}><h2>{editingId ? "Edit shortcut" : "Create shortcut"}</h2><label>Name<input aria-label="Shortcut name" maxLength={100} value={draft.name} onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))} placeholder="e.g. Weekly report" /></label><label>Workspace resource<select aria-label="Shortcut resource" value={draft.resourceId} onChange={(event) => setDraft((current) => ({ ...current, resourceId: event.target.value, targetId: "" }))}><option value="">Choose a resource</option>{resources.map((resource) => <option key={resource.id} value={resource.id}>{resource.displayName}</option>)}</select></label><label>Target<select aria-label="Shortcut target" value={draft.targetId} disabled={!selectedResource} onChange={(event) => setDraft((current) => ({ ...current, targetId: event.target.value }))}><option value="">Default active target</option>{eligibleTargets.map((target) => <option key={target.id} value={target.id}>{target.kind} · {target.canonicalUri}</option>)}</select></label><p className="smart-collection-preview">Quick Launch will send the selected resource and optional target through the normal app launch flow.</p>{error && <p role="alert" className="form-error">{error}</p>}{status && <p role="status">{status}</p>}<div className="resource-editor-actions"><button type="button" className="button secondary" disabled={busy} onClick={startNew}>Clear form</button><button type="submit" className="button primary" disabled={busy || !resources.length}>{busy ? "Saving…" : "Save shortcut"}</button></div></form>
    </div>
  </section>;
}
