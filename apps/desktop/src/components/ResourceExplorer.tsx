import { useEffect, useMemo, useRef, useState } from "react";
import type { ResourcePatch, ResourceRelocationPreview, ResourceRelocationPreviewRequest, ResourceRelocationResult } from "@sw/contracts";
import type { Resource } from "../lib/contracts";
import { Icon } from "./Icon";

type SortKey = "name" | "type" | "category" | "location" | "launches" | "lastLaunched";
type Column = { key: SortKey; label: string };
const columns: Column[] = [{ key: "name", label: "Name" }, { key: "type", label: "Type" }, { key: "category", label: "Collection" }, { key: "location", label: "Location" }, { key: "launches", label: "Launches" }, { key: "lastLaunched", label: "Last launched" }];
export function buildResourceCsv(resources: Resource[], visible: SortKey[]) {
  const selectedColumns = columns.filter((column) => visible.includes(column.key));
  const cell = (value: unknown) => {
    let text = String(value ?? "");
    if (/^\s*[=+\-@]/.test(text)) text = `'${text}`;
    return `"${text.replaceAll('"', '""')}"`;
  };
  return [selectedColumns.map((column) => cell(column.label)).join(","), ...resources.map((resource) => selectedColumns.map(({ key }) => cell(valueFor(resource, key))).join(","))].join("\r\n");
}
function valueFor(resource: Resource, key: SortKey) {
  switch (key) {
    case "name": return resource.displayName;
    case "type": return resource.resourceType;
    case "category": return resource.category || "";
    case "location": return resource.targets.map((target) => target.canonicalUri).join(" | ");
    case "launches": return resource.launchCount;
    case "lastLaunched": return resource.lastLaunchedUtc || "";
  }
}

function RelocationAssistant({ profileId, resources, selectedIds, onPreview, onCommit }: { profileId: string; resources: Resource[]; selectedIds: string[]; onPreview: (request: ResourceRelocationPreviewRequest) => Promise<ResourceRelocationPreview>; onCommit: (preview: ResourceRelocationPreview) => Promise<ResourceRelocationResult> }) {
  const [sourcePrefix, setSourcePrefix] = useState("");
  const [destinationPrefix, setDestinationPrefix] = useState("");
  const [selectedOnly, setSelectedOnly] = useState(false);
  const [preview, setPreview] = useState<ResourceRelocationPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const selectedSignature = selectedIds.join("\0");
  const previousSelectedSignature = useRef(selectedSignature);
  useEffect(() => {
    if (previousSelectedSignature.current !== selectedSignature) {
      previousSelectedSignature.current = selectedSignature;
      setPreview(null); setError(""); setMessage("");
    }
  }, [selectedSignature]);
  const invalidate = () => { setPreview(null); setError(""); setMessage(""); };
  const requestPreview = async () => {
    setError(""); setMessage(""); setPreview(null);
    if (!sourcePrefix.trim() || !destinationPrefix.trim()) { setError("Enter both the current path prefix and the new path prefix."); return; }
    if (selectedOnly && !selectedIds.length) { setError("Select at least one resource in the table first."); return; }
    setBusy(true);
    try {
      const result = await onPreview({ profileId, sourcePrefix: sourcePrefix.trim(), destinationPrefix: destinationPrefix.trim(), ...(selectedOnly ? { resourceIds: selectedIds } : {}) });
      setPreview(result);
      setMessage(result.changes.length ? `Review ${result.changes.length} target path change${result.changes.length === 1 ? "" : "s"} below.` : "No matching file or folder targets were found.");
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Could not preview target relocation."); }
    finally { setBusy(false); }
  };
  const applyPreview = async () => {
    if (!preview) return;
    setError(""); setMessage(""); setBusy(true);
    try {
      const result = await onCommit(preview);
      setPreview(null);
      setMessage(`${result.updatedTargetCount} saved target path${result.updatedTargetCount === 1 ? "" : "s"} updated. Files and folders were not moved.`);
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Could not apply the reviewed target path changes."); }
    finally { setBusy(false); }
  };
  const resourceNames = new Map(resources.map((resource) => [resource.id, resource.displayName]));
  return <section className="relocation-assistant" aria-label="Relocation assistant">
    <header><div><h2>Relocate saved target paths</h2><p>Preview stored file and folder path changes before confirming them.</p></div></header>
    <p className="relocation-safety">This updates launcher references only. It does not move, copy, scan, or delete files or folders.</p>
    {error && <p role="alert" className="form-error">{error}</p>}{message && <p role="status" className="relocation-status">{message}</p>}
    <div className="relocation-fields">
      <label>Current path prefix<input aria-label="Current path prefix" value={sourcePrefix} disabled={busy} onChange={(event) => { setSourcePrefix(event.target.value); invalidate(); }} placeholder="C:\\Old location" /></label>
      <label>New path prefix<input aria-label="New path prefix" value={destinationPrefix} disabled={busy} onChange={(event) => { setDestinationPrefix(event.target.value); invalidate(); }} placeholder="D:\\New location" /></label>
      <label className="relocation-scope"><input type="checkbox" checked={selectedOnly} disabled={busy || selectedIds.length === 0} onChange={(event) => { setSelectedOnly(event.target.checked); invalidate(); }} /> Limit to {selectedIds.length || "selected"} resource{selectedIds.length === 1 ? "" : "s"} in the table</label>
      <button className="button secondary" disabled={busy} onClick={() => void requestPreview()}>{busy && !preview ? "Preparing preview…" : "Preview path changes"}</button>
    </div>
    {preview && <div className="relocation-preview" aria-label="Relocation preview"><h3>Review proposed changes ({preview.changes.length})</h3>
      {preview.changes.length > 0 ? <ul>{preview.changes.map((change) => <li key={`${change.resourceId}:${change.targetId}`}><b>{resourceNames.get(change.resourceId) || "Resource"}</b><span className="relocation-path">{change.fromUri}</span><span aria-hidden="true">→</span><span className="relocation-path">{change.toUri}</span></li>)}</ul> : <p>No saved file or folder target begins with that path prefix.</p>}
      <button className="button primary" disabled={busy || preview.changes.length === 0} onClick={() => void applyPreview()}>{busy ? "Applying reviewed changes…" : `Apply ${preview.changes.length} path change${preview.changes.length === 1 ? "" : "s"}`}</button>
    </div>}
  </section>;
}

export function ResourceExplorer({ resources, profileId = "default", onSelect, onAdd, onUpdate, onDuplicate, onRelocationPreview, onRelocationCommit }: { resources: Resource[]; profileId?: string; onSelect: (resource: Resource) => void; onAdd: () => void; onUpdate: (id: string, patch: ResourcePatch) => boolean | void | Promise<boolean | void>; onDuplicate?: (resource: Resource) => Promise<Resource>; onRelocationPreview?: (request: ResourceRelocationPreviewRequest) => Promise<ResourceRelocationPreview>; onRelocationCommit?: (preview: ResourceRelocationPreview) => Promise<ResourceRelocationResult> }) {
  const [query, setQuery] = useState("");
  const [type, setType] = useState("all");
  const [collection, setCollection] = useState("all");
  const [favoritesOnly, setFavoritesOnly] = useState(false);
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [ascending, setAscending] = useState(true);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [visible, setVisible] = useState<SortKey[]>(columns.map(({ key }) => key));
  const [message, setMessage] = useState("");
  const [duplicateBusyId, setDuplicateBusyId] = useState<string | null>(null);
  const [relocationOpen, setRelocationOpen] = useState(false);
  const collections = [...new Set(resources.map((resource) => resource.category).filter((value): value is string => Boolean(value)))].sort((a, b) => a.localeCompare(b));
  const filtered = useMemo(() => resources.filter((resource) => {
    const haystack = [resource.displayName, resource.description, resource.category, ...resource.tags, ...resource.aliases, ...resource.targets.map((target) => target.canonicalUri)].join(" ").toLocaleLowerCase();
    return (!query.trim() || haystack.includes(query.trim().toLocaleLowerCase())) && (type === "all" || resource.resourceType === type) && (collection === "all" || resource.category === collection) && (!favoritesOnly || resource.favorite);
  }).sort((a, b) => {
    const left = valueFor(a, sortKey); const right = valueFor(b, sortKey);
    const order = typeof left === "number" && typeof right === "number" ? left - right : String(left).localeCompare(String(right), undefined, { numeric: true, sensitivity: "base" });
    return (ascending ? order : -order) || a.id.localeCompare(b.id);
  }), [resources, query, type, collection, favoritesOnly, sortKey, ascending]);
  const selectAll = (checked: boolean) => setSelected((current) => {
    const next = new Set(current);
    for (const resource of filtered) checked ? next.add(resource.id) : next.delete(resource.id);
    return next;
  });
  const changeSort = (key: SortKey) => { if (sortKey === key) setAscending((value) => !value); else { setSortKey(key); setAscending(true); } };
  const exportCsv = () => {
    const blob = new Blob(["\uFEFF", buildResourceCsv(filtered, visible)], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob); const anchor = document.createElement("a"); anchor.href = url; anchor.download = "workspace-resources.csv"; anchor.click(); URL.revokeObjectURL(url);
    setMessage(`Exported ${filtered.length} filtered resources as CSV.`);
  };
  const duplicate = async (resource: Resource) => {
    if (!onDuplicate) return;
    setDuplicateBusyId(resource.id); setMessage("");
    try {
      const copy = await onDuplicate(resource);
      setMessage(`“${copy.displayName}” was added to this profile. Its target files were not copied.`);
    } catch (reason) { setMessage(reason instanceof Error ? `Could not duplicate resource: ${reason.message}` : "Could not duplicate this resource."); }
    finally { setDuplicateBusyId(null); }
  };
  const [bulkSaving, setBulkSaving] = useState(false);
  const addFavorites = async () => {
    const chosen = resources.filter((resource) => selected.has(resource.id) && !resource.favorite);
    setBulkSaving(true);
    const outcomes = await Promise.all(chosen.map(async (resource) => {
      try { return await onUpdate(resource.id, { favorite: true }) !== false; }
      catch { return false; }
    }));
    const succeeded = chosen.filter((_, index) => outcomes[index]);
    setSelected((current) => new Set([...current].filter((id) => !succeeded.some((resource) => resource.id === id))));
    const count = succeeded.length;
    setMessage(`${count} of ${chosen.length} selected resource${chosen.length === 1 ? "" : "s"} added to Favorites${count < chosen.length ? "; failed items remain selected for retry." : "."}`);
    setBulkSaving(false);
  };
  const visibleRows = filtered;
  return <section className="resource-explorer" aria-label="Resource Explorer">
    <div className="welcome-row"><div><h1>Resource Explorer</h1><p>Search, filter, sort and select workspace resources.</p></div><div className="explorer-actions"><button className="button secondary" onClick={exportCsv} disabled={!filtered.length}>Export CSV</button><button className="button primary" onClick={onAdd}><Icon name="plus" size={16}/> Add resource</button></div></div>
    <div className="explorer-toolbar"><label className="search-box explorer-search"><Icon name="search" size={18}/><input aria-label="Search resources" placeholder="Search by name, tag, alias or location" value={query} onChange={(event) => setQuery(event.target.value)} /></label><label>Type<select aria-label="Filter by type" value={type} onChange={(event) => setType(event.target.value)}><option value="all">All types</option>{["file", "folder", "app", "shortcut", "url"].map((value) => <option key={value}>{value}</option>)}</select></label><label>Collection<select aria-label="Filter by collection" value={collection} onChange={(event) => setCollection(event.target.value)}><option value="all">All collections</option>{collections.map((value) => <option key={value}>{value}</option>)}</select></label><label className="explorer-check"><input type="checkbox" checked={favoritesOnly} onChange={(event) => setFavoritesOnly(event.target.checked)} /> Favorites</label><details className="column-picker"><summary>Columns</summary><div>{columns.map((column) => <label key={column.key}><input type="checkbox" checked={visible.includes(column.key)} onChange={(event) => setVisible((current) => event.target.checked ? [...current, column.key] : current.filter((key) => key !== column.key))} />{column.label}</label>)}</div></details><button className="button secondary" onClick={() => { setRelocationOpen((open) => !open); setMessage(""); }} aria-expanded={relocationOpen}>{relocationOpen ? "Close relocation" : "Relocate paths"}</button></div>
    {relocationOpen && onRelocationPreview && onRelocationCommit && <RelocationAssistant profileId={profileId} resources={resources} selectedIds={[...selected]} onPreview={onRelocationPreview} onCommit={onRelocationCommit} />}
    <div className="explorer-bulk" aria-live="polite"><span>{filtered.length} resources{selected.size ? ` · ${selected.size} selected` : ""}</span>{selected.size > 0 && <button className="text-button" disabled={bulkSaving} onClick={() => void addFavorites()}>{bulkSaving ? "Updating…" : "Add selected to Favorites"}</button>}{message && <span role="status">{message}</span>}</div>
    <div className="table-shell"><table><thead><tr><th className="select-column"><input type="checkbox" aria-label="Select all filtered resources" checked={filtered.length > 0 && filtered.every((resource) => selected.has(resource.id))} onChange={(event) => selectAll(event.target.checked)} /></th>{columns.filter((column) => visible.includes(column.key)).map((column) => <th key={column.key} aria-sort={sortKey === column.key ? ascending ? "ascending" : "descending" : "none"}><button className="sort-heading" onClick={() => changeSort(column.key)}>{column.label}{sortKey === column.key && <span aria-hidden="true">{ascending ? " ↑" : " ↓"}</span>}</button></th>)}<th>Action</th></tr></thead><tbody>{visibleRows.map((resource) => <tr key={resource.id} onClick={() => onSelect(resource)} tabIndex={0} onKeyDown={(event) => event.key === "Enter" && onSelect(resource)}><td className="select-column" onClick={(event) => event.stopPropagation()}><input type="checkbox" aria-label={`Select ${resource.displayName}`} checked={selected.has(resource.id)} onChange={(event) => setSelected((current) => { const next = new Set(current); event.target.checked ? next.add(resource.id) : next.delete(resource.id); return next; })} /></td>{columns.filter((column) => visible.includes(column.key)).map((column) => <td key={column.key} className={column.key === "location" ? "table-location" : undefined}>{column.key === "name" ? <span className="table-name"><span className={`resource-mark ${resource.resourceType}`} aria-hidden="true">{resource.displayName.slice(0, 1).toUpperCase()}</span><b>{resource.displayName}</b></span> : column.key === "lastLaunched" ? resource.lastLaunchedUtc ? new Date(resource.lastLaunchedUtc).toLocaleString() : "Never" : String(valueFor(resource, column.key) || (column.key === "launches" ? 0 : "—"))}</td>)}<td><button className="text-button" onClick={(event) => { event.stopPropagation(); onSelect(resource); }}>Edit details</button>{onDuplicate && <button className="text-button" aria-label={`Duplicate ${resource.displayName}`} disabled={duplicateBusyId === resource.id} onClick={(event) => { event.stopPropagation(); void duplicate(resource); }}>{duplicateBusyId === resource.id ? "Duplicating…" : "Duplicate"}</button>}</td></tr>)}</tbody></table>{filtered.length === 0 && <div className="table-empty">No resources match these filters. Clear a filter or add a resource.</div>}</div>
  </section>;
}
