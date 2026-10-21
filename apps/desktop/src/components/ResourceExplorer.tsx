import { useMemo, useState } from "react";
import type { ResourcePatch } from "@sw/contracts";
import type { Resource } from "../lib/contracts";
import { Icon } from "./Icon";

type SortKey = "name" | "type" | "category" | "location" | "launches" | "lastLaunched";
type Column = { key: SortKey; label: string };
const columns: Column[] = [{ key: "name", label: "Name" }, { key: "type", label: "Type" }, { key: "category", label: "Collection" }, { key: "location", label: "Location" }, { key: "launches", label: "Launches" }, { key: "lastLaunched", label: "Last launched" }];
export function buildResourceCsv(resources: Resource[], visible: SortKey[]) {
  const selectedColumns = columns.filter((column) => visible.includes(column.key));
  const cell = (value: unknown) => {
    let text = String(value ?? "");
    if (/^[=+\-@\t\r]/.test(text)) text = `'${text}`;
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
export function ResourceExplorer({ resources, onSelect, onAdd, onUpdate }: { resources: Resource[]; onSelect: (resource: Resource) => void; onAdd: () => void; onUpdate: (id: string, patch: ResourcePatch) => boolean | void | Promise<boolean | void> }) {
  const [query, setQuery] = useState("");
  const [type, setType] = useState("all");
  const [collection, setCollection] = useState("all");
  const [favoritesOnly, setFavoritesOnly] = useState(false);
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [ascending, setAscending] = useState(true);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [visible, setVisible] = useState<SortKey[]>(columns.map(({ key }) => key));
  const [message, setMessage] = useState("");
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
    <div className="explorer-toolbar"><label className="search-box explorer-search"><Icon name="search" size={18}/><input aria-label="Search resources" placeholder="Search by name, tag, alias or location" value={query} onChange={(event) => setQuery(event.target.value)} /></label><label>Type<select aria-label="Filter by type" value={type} onChange={(event) => setType(event.target.value)}><option value="all">All types</option>{["file", "folder", "app", "shortcut", "url"].map((value) => <option key={value}>{value}</option>)}</select></label><label>Collection<select aria-label="Filter by collection" value={collection} onChange={(event) => setCollection(event.target.value)}><option value="all">All collections</option>{collections.map((value) => <option key={value}>{value}</option>)}</select></label><label className="explorer-check"><input type="checkbox" checked={favoritesOnly} onChange={(event) => setFavoritesOnly(event.target.checked)} /> Favorites</label><details className="column-picker"><summary>Columns</summary><div>{columns.map((column) => <label key={column.key}><input type="checkbox" checked={visible.includes(column.key)} onChange={(event) => setVisible((current) => event.target.checked ? [...current, column.key] : current.filter((key) => key !== column.key))} />{column.label}</label>)}</div></details></div>
    <div className="explorer-bulk" aria-live="polite"><span>{filtered.length} resources{selected.size ? ` · ${selected.size} selected` : ""}</span>{selected.size > 0 && <button className="text-button" disabled={bulkSaving} onClick={() => void addFavorites()}>{bulkSaving ? "Updating…" : "Add selected to Favorites"}</button>}{message && <span role="status">{message}</span>}</div>
    <div className="table-shell"><table><thead><tr><th className="select-column"><input type="checkbox" aria-label="Select all filtered resources" checked={filtered.length > 0 && filtered.every((resource) => selected.has(resource.id))} onChange={(event) => selectAll(event.target.checked)} /></th>{columns.filter((column) => visible.includes(column.key)).map((column) => <th key={column.key} aria-sort={sortKey === column.key ? ascending ? "ascending" : "descending" : "none"}><button className="sort-heading" onClick={() => changeSort(column.key)}>{column.label}{sortKey === column.key && <span aria-hidden="true">{ascending ? " ↑" : " ↓"}</span>}</button></th>)}<th>Action</th></tr></thead><tbody>{visibleRows.map((resource) => <tr key={resource.id} onClick={() => onSelect(resource)} tabIndex={0} onKeyDown={(event) => event.key === "Enter" && onSelect(resource)}><td className="select-column" onClick={(event) => event.stopPropagation()}><input type="checkbox" aria-label={`Select ${resource.displayName}`} checked={selected.has(resource.id)} onChange={(event) => setSelected((current) => { const next = new Set(current); event.target.checked ? next.add(resource.id) : next.delete(resource.id); return next; })} /></td>{columns.filter((column) => visible.includes(column.key)).map((column) => <td key={column.key} className={column.key === "location" ? "table-location" : undefined}>{column.key === "name" ? <span className="table-name"><span className={`resource-mark ${resource.resourceType}`} aria-hidden="true">{resource.displayName.slice(0, 1).toUpperCase()}</span><b>{resource.displayName}</b></span> : column.key === "lastLaunched" ? resource.lastLaunchedUtc ? new Date(resource.lastLaunchedUtc).toLocaleString() : "Never" : String(valueFor(resource, column.key) || (column.key === "launches" ? 0 : "—"))}</td>)}<td><button className="text-button" onClick={(event) => { event.stopPropagation(); onSelect(resource); }}>Edit details</button></td></tr>)}</tbody></table>{filtered.length === 0 && <div className="table-empty">No resources match these filters. Clear a filter or add a resource.</div>}</div>
  </section>;
}
