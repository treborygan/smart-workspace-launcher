import { useState } from "react";
import type { FormEvent } from "react";
import type { ResourcePatch, TargetKind } from "@sw/contracts";
import type { Resource } from "../lib/contracts";

type EditableTarget = { id: string; kind: TargetKind; canonicalUri: string; priority: number; arguments: string[]; workingDirectory?: string | null; active: boolean };
export function ResourceEditorForm({ resource, onCancel, onSave }: { resource: Resource; onCancel: () => void; onSave: (patch: ResourcePatch) => Promise<void> }) {
  const [name, setName] = useState(resource.displayName);
  const [description, setDescription] = useState(resource.description || "");
  const [category, setCategory] = useState(resource.category || "");
  const [tags, setTags] = useState(resource.tags.join("\n"));
  const [aliases, setAliases] = useState(resource.aliases.join("\n"));
  const [targets, setTargets] = useState<EditableTarget[]>(resource.targets.map((target) => ({ ...target, arguments: [...target.arguments] })));
  const [error, setError] = useState(""); const [saving, setSaving] = useState(false);
  const submit = async (event: FormEvent) => {
    event.preventDefault(); setError("");
    if (!name.trim()) { setError("Resource name cannot be blank."); return; }
    if (targets.some((target) => !target.canonicalUri.trim())) { setError("Each saved target needs a location."); return; }
    setSaving(true);
    try { await onSave({ displayName: name.trim(), description: description.trim() || null, category: category.trim() || undefined, tags: splitLines(tags), aliases: splitLines(aliases), targets: targets.map((target) => ({ ...target, canonicalUri: target.canonicalUri.trim() })) }); onCancel(); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Could not save resource changes."); }
    finally { setSaving(false); }
  };
  return <form className="resource-editor" aria-label={`Edit ${resource.displayName}`} onSubmit={(event) => void submit(event)}>
    <label>Name<input value={name} onChange={(event) => setName(event.target.value)} /></label>
    <label>Description<input value={description} onChange={(event) => setDescription(event.target.value)} /></label>
    <label>Collection<input value={category} onChange={(event) => setCategory(event.target.value)} placeholder="Uncategorized" /></label>
    <label>Tags · one per line<textarea value={tags} onChange={(event) => setTags(event.target.value)} rows={3} /></label>
    <label>Aliases · one per line<textarea value={aliases} onChange={(event) => setAliases(event.target.value)} rows={2} /></label>
    <fieldset><legend>Targets</legend>{targets.map((target, index) => <div className="resource-editor-target" key={target.id}><label>Target {index + 1}<input aria-label={`Target ${index + 1} location`} value={target.canonicalUri} onChange={(event) => setTargets((current) => current.map((item) => item.id === target.id ? { ...item, canonicalUri: event.target.value } : item))} /></label><label className="target-kind-field">Type<select aria-label={`Target ${index + 1} type`} value={target.kind} onChange={(event) => setTargets((current) => current.map((item) => item.id === target.id ? { ...item, kind: event.target.value as TargetKind } : item))}>{["file", "folder", "app", "shortcut", "url"].map((kind) => <option key={kind}>{kind}</option>)}</select></label><label className="target-active"><input type="checkbox" aria-label={`Target ${index + 1} active`} checked={target.active} onChange={(event) => setTargets((current) => current.map((item) => item.id === target.id ? { ...item, active: event.target.checked } : item))} /> Enabled</label>{targets.length > 1 && <button className="subtle-action danger" type="button" onClick={() => setTargets((current) => current.filter((item) => item.id !== target.id))}>Remove target</button>}</div>)}<button className="subtle-action" type="button" onClick={() => setTargets((current) => [...current, { id: crypto.randomUUID(), kind: resource.resourceType, canonicalUri: "", priority: current.length, arguments: [], active: true }])}>Add alternate target</button></fieldset>
    {error && <p role="alert" className="form-error">{error}</p>}<div className="resource-editor-actions"><button type="button" className="button secondary" onClick={onCancel} disabled={saving}>Cancel</button><button type="submit" className="button primary" disabled={saving}>{saving ? "Saving…" : "Save changes"}</button></div>
  </form>;
}
function splitLines(value: string) { return [...new Set(value.split(/\r?\n/).map((item) => item.trim()).filter(Boolean))]; }
