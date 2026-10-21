import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import type { Resource } from "../lib/contracts";
import type { SmartCollection, SmartCollectionFilters } from "../lib/smartCollections";
import { matchesSmartCollection, validateSmartCollection } from "../lib/smartCollections";

type Draft = { name: string; filters: SmartCollectionFilters };
const emptyDraft: Draft = { name: "", filters: {} };

export function SmartCollectionsManager({ profileId, collections, resources, onSave, onDelete, onClose }: {
  profileId: string;
  collections: SmartCollection[];
  resources: Resource[];
  onSave: (collection: SmartCollection) => Promise<SmartCollection>;
  onDelete: (collectionId: string) => Promise<void>;
  onClose: () => void;
}) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft>(emptyDraft);
  const [error, setError] = useState("");
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (!editingId) return;
    const existing = collections.find((item) => item.id === editingId);
    if (existing) setDraft({ name: existing.name, filters: { ...existing.filters } });
  }, [editingId, collections]);
  const count = resources.filter((resource) => matchesSmartCollection(resource, draft.filters)).length;
  const save = async (event: FormEvent) => {
    event.preventDefault(); setError(""); setStatus("");
    const validation = validateSmartCollection(draft.name, draft.filters);
    if (validation) { setError(validation); return; }
    setBusy(true);
    try {
      await onSave({ id: editingId || crypto.randomUUID(), profileId, name: draft.name.trim(), filters: compactFilters(draft.filters) });
      setEditingId(null); setDraft(emptyDraft); setStatus("Smart collection saved.");
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Could not save this collection."); }
    finally { setBusy(false); }
  };
  const remove = async (item: SmartCollection) => {
    if (!window.confirm(`Delete the smart collection “${item.name}”? Resources will not be changed.`)) return;
    setBusy(true); setError("");
    try { await onDelete(item.id); if (editingId === item.id) { setEditingId(null); setDraft(emptyDraft); } setStatus(`${item.name} deleted.`); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Could not delete this collection."); }
    finally { setBusy(false); }
  };
  const setFilter = <K extends keyof SmartCollectionFilters>(key: K, value: SmartCollectionFilters[K] | undefined) => setDraft((current) => {
    const filters = { ...current.filters };
    if (value === undefined || value === "" || value === false) delete filters[key];
    else Object.assign(filters, { [key]: value });
    return { ...current, filters };
  });
  return <section className="smart-collections-manager" aria-label="Smart Collections manager">
    <div className="welcome-row"><div><h1>Smart Collections</h1><p>Save filters that update as resources change. Collections never modify resources.</p></div><button className="button secondary" onClick={onClose}>Done</button></div>
      <div className="smart-collections-layout"><section className="smart-collection-list" aria-label="Saved smart collections"><div className="smart-collection-list-head"><h2>Saved collections</h2><button className="button primary" onClick={() => { setEditingId(null); setDraft(emptyDraft); setError(""); }}>New collection</button></div>{collections.length ? collections.map((item) => <article className="smart-collection-row" key={item.id}><button className="smart-collection-edit" onClick={() => { setError(""); setStatus(""); setEditingId(item.id); }}><b>{item.name}</b><small>{Object.keys(item.filters).length || "All"} filters · {resources.filter((resource) => matchesSmartCollection(resource, item.filters)).length} resources</small></button><button className="text-button danger" disabled={busy} onClick={() => void remove(item)}>Delete</button></article>) : <p className="empty-state">No saved smart collections yet.</p>}</section>
      <form className="smart-collection-form" aria-label={editingId ? "Edit smart collection" : "New smart collection"} onSubmit={(event) => void save(event)}><h2>{editingId ? "Edit collection" : "Create collection"}</h2><label>Name<input aria-label="Collection name" maxLength={100} value={draft.name} onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))} placeholder="e.g. Favorite spreadsheets" /></label><label>Resource type<select aria-label="Resource type filter" value={draft.filters.resourceType || ""} onChange={(event) => setFilter("resourceType", event.target.value ? event.target.value as Resource["resourceType"] : undefined)}><option value="">Any type</option>{["file", "folder", "app", "shortcut", "url"].map((value) => <option key={value}>{value}</option>)}</select></label><label>Category<input aria-label="Category filter" maxLength={80} value={draft.filters.category || ""} onChange={(event) => setFilter("category", event.target.value || undefined)} placeholder="Any category" /></label><label>Tag<input aria-label="Tag filter" maxLength={80} value={draft.filters.tag || ""} onChange={(event) => setFilter("tag", event.target.value || undefined)} placeholder="Any tag" /></label><label>Text contains<input aria-label="Text filter" maxLength={200} value={draft.filters.text || ""} onChange={(event) => setFilter("text", event.target.value || undefined)} placeholder="Name, alias, location…" /></label><label className="smart-collection-favorite"><input type="checkbox" checked={draft.filters.favoriteOnly || false} onChange={(event) => setFilter("favoriteOnly", event.target.checked || undefined)} /> Favorites only</label><p className="smart-collection-preview" aria-live="polite">Matches {count} of {resources.length} current resources. Filters use AND logic.</p>{error && <p role="alert" className="form-error">{error}</p>}{status && <p role="status">{status}</p>}<div className="resource-editor-actions"><button type="button" className="button secondary" disabled={busy} onClick={() => { setEditingId(null); setDraft(emptyDraft); setError(""); }}>Clear form</button><button type="submit" className="button primary" disabled={busy}>{busy ? "Saving…" : "Save collection"}</button></div></form>
    </div>
  </section>;
}

function compactFilters(filters: SmartCollectionFilters): SmartCollectionFilters {
  return Object.fromEntries(Object.entries(filters).filter(([, value]) => value !== undefined && value !== "" && value !== false)) as SmartCollectionFilters;
}
