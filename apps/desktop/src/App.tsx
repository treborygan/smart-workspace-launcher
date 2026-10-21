import { useEffect, useMemo, useState } from "react";
import type { FormEvent } from "react";
import { Icon } from "./components/Icon";
import { api } from "./lib/api";
import type { ResourcePatch } from "@sw/contracts";
import type { Profile, Resource } from "./lib/contracts";
import "./styles.css";

type Section = "Dashboard" | "Quick Launch" | "Explorer" | "Analytics";
const nav = [
  { label: "Dashboard" as const, icon: "grid" as const },
  { label: "Quick Launch" as const, icon: "search" as const },
  { label: "Explorer" as const, icon: "folder" as const },
  { label: "Analytics" as const, icon: "chart" as const },
];

function ResourceMark({ resource }: { resource: Resource }) {
  const ext = resource.displayName.split(".").pop()?.toLowerCase();
  const mark = resource.resourceType === "folder" ? "▱" : resource.resourceType === "url" ? "◎" : ext === "xlsx" ? "X" : ext === "pptx" ? "P" : resource.displayName.slice(0, 1).toUpperCase();
  const color = resource.resourceType === "folder" ? "folder" : ext === "xlsx" ? "excel" : resource.resourceType === "url" ? "web" : "app";
  return <span className={`resource-mark ${color}`} aria-hidden="true">{mark}</span>;
}

function AddResourceDialog({ onClose, onCreate }: { onClose: () => void; onCreate: (resource: Resource) => void }) {
  const [name, setName] = useState("");
  const [uri, setUri] = useState("");
  const [kind, setKind] = useState<"file" | "folder" | "app">("file");
  const [error, setError] = useState("");
  const submit = async (event: FormEvent) => {
    event.preventDefault(); setError("");
    if (!name.trim() || !uri.trim()) { setError("Add a name and a file path or HTTPS address."); return; }
    if (/^https?:\/\//i.test(uri) && !/^https:\/\//i.test(uri)) { setError("Only HTTPS addresses are supported."); return; }
    try {
      const resource = await api.create({ resourceType: /^https:\/\//i.test(uri) ? "url" : kind, displayName: name.trim(), defaultAction: "open", targets: [{ id: crypto.randomUUID(), kind: /^https:\/\//i.test(uri) ? "url" : kind, canonicalUri: uri.trim(), priority: 0, arguments: [], active: true }] });
      onCreate(resource); onClose();
    } catch (e) { setError(e instanceof Error ? e.message : "Could not save this resource."); }
  };
  return <div className="modal-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="dialog" role="dialog" aria-modal="true" aria-labelledby="dialog-title">
      <header className="dialog-heading"><div><h2 id="dialog-title">Add a resource</h2><p>Save a file, folder, application or secure web address.</p></div><button className="icon-button" onClick={onClose} aria-label="Close"><Icon name="close" /></button></header>
      <form onSubmit={submit} className="resource-form">
        <label>Display name<input autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Project roadmap" /></label>
        <label>Type<select value={kind} onChange={(e) => setKind(e.target.value as "file" | "folder" | "app")}><option value="file">File</option><option value="folder">Folder</option><option value="app">Application</option></select></label>
        <label>Location<input value={uri} onChange={(e) => setUri(e.target.value)} placeholder="C:\\Work\\Roadmap.xlsx or https://…" /></label>
        {error && <p className="form-error" role="alert">{error}</p>}
        <footer><button type="button" className="button secondary" onClick={onClose}>Cancel</button><button className="button primary" type="submit"><Icon name="plus" size={16} /> Add resource</button></footer>
      </form>
    </section>
  </div>;
}

export default function App() {
  const [resources, setResources] = useState<Resource[]>([]);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [section, setSection] = useState<Section>("Dashboard");
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("All resources");
  const [showAdd, setShowAdd] = useState(false);
  const [notice, setNotice] = useState("");
  const [loading, setLoading] = useState(true);
  const [dragId, setDragId] = useState<string | null>(null);
  const [cardWidths, setCardWidths] = useState<Record<string, number>>(() => {
    try { return JSON.parse(localStorage.getItem("swl-card-widths-v1") || "{}"); } catch { return {}; }
  });
  const [dark, setDark] = useState(false);

  const reload = async () => {
    try { const [items, activeProfiles] = await Promise.all([api.resources(), api.profiles()]); const order = JSON.parse(localStorage.getItem(`swl-layout-order-${activeProfiles[0]?.id || "default"}`) || "[]") as string[]; const rank = new Map(order.map((id, index) => [id, index])); const ordered = [...items].sort((a, b) => (rank.get(a.id) ?? Number.MAX_SAFE_INTEGER) - (rank.get(b.id) ?? Number.MAX_SAFE_INTEGER)); setResources(ordered); setProfiles(activeProfiles); setSelectedId((old) => old && items.some((r) => r.id === old) ? old : items[0]?.id ?? null); }
    catch (e) { setNotice(e instanceof Error ? e.message : "Could not connect to the local workspace."); }
    finally { setLoading(false); }
  };
  useEffect(() => { void reload(); }, []);
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if ((event.altKey && event.code === "Space") || ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k")) {
        event.preventDefault(); setSection("Quick Launch"); window.setTimeout(() => document.querySelector<HTMLInputElement>("#global-search")?.focus(), 10);
      }
      if (event.key === "Escape") { setShowAdd(false); setQuery(""); }
    };
    window.addEventListener("keydown", handler); return () => window.removeEventListener("keydown", handler);
  }, []);
  useEffect(() => { document.documentElement.dataset.theme = dark ? "dark" : "light"; }, [dark]);

  const filtered = useMemo(() => resources.filter((resource) => {
    const haystack = [resource.displayName, resource.description, resource.category, ...(resource.tags || []), ...resource.targets.map((t) => t.canonicalUri)].join(" ").toLowerCase();
    const matchQuery = query.trim().length === 0 || haystack.includes(query.trim().toLowerCase());
    const matchCategory = category === "All resources" || (category === "Favorites" ? resource.favorite : resource.category === category);
    return matchQuery && matchCategory;
  }), [resources, query, category]);
  const selected = resources.find((resource) => resource.id === selectedId) ?? null;

  const update = async (id: string, patch: ResourcePatch) => {
    try { const changed = await api.update(id, patch); setResources((current) => current.map((r) => r.id === id ? changed : r)); }
    catch (e) { setNotice(e instanceof Error ? e.message : "Could not update resource."); }
  };
  const launch = async (resource: Resource) => {
    try {
      const result = await api.launch(resource.id);
      if (result.status === "failed") throw new Error(result.errorCode || "The system could not hand off this resource.");
      setResources((current) => current.map((r) => r.id === resource.id ? { ...r, launchCount: (r.launchCount || 0) + 1, lastLaunchedUtc: result.launchedAtUtc } : r));
      setNotice(result.status === "handed_off" ? `Windows accepted the launch request for ${resource.displayName}.` : "Launch request sent.");
    } catch (e) { setNotice(e instanceof Error ? e.message : "Launch failed."); }
  };
  const remove = async (resource: Resource) => {
    if (!window.confirm(`Remove “${resource.displayName}” from this workspace?`)) return;
    try { await api.remove(resource.id); setResources((all) => all.filter((r) => r.id !== resource.id)); setSelectedId(null); }
    catch (e) { setNotice(e instanceof Error ? e.message : "Could not remove resource."); }
  };
  const reorder = (overId: string) => {
    if (!dragId || dragId === overId) return;
    const next = [...resources]; const from = next.findIndex((r) => r.id === dragId); const to = next.findIndex((r) => r.id === overId);
    const [moved] = next.splice(from, 1); next.splice(to, 0, moved); setResources(next); setDragId(null);
    localStorage.setItem(`swl-layout-order-${profiles[0]?.id || "default"}`, JSON.stringify(next.map((r) => r.id)));
  };
  const resizeCard = (id: string, width: number) => {
    const next = { ...cardWidths, [id]: width }; setCardWidths(next); localStorage.setItem("swl-card-widths-v1", JSON.stringify(next));
  };

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><span className="brand-symbol"><span /><span /><span /><span /></span><span className="brand-name">Workspace</span></div>
      <div className="profile-select"><span className="profile-avatar">{profiles[0]?.name?.slice(0, 1) || "P"}</span><span className="profile-label"><b>{profiles[0]?.name || "Personal"}</b><small>Workspace profile</small></span><Icon name="chevron" size={15} /></div>
      <div className="nav-label">WORKSPACE</div>
      <nav aria-label="Primary navigation">{nav.map((item) => <button key={item.label} className={`nav-item ${section === item.label ? "active" : ""}`} onClick={() => setSection(item.label)}><Icon name={item.icon} size={18} />{item.label}{item.label === "Quick Launch" && <kbd>⌥ Space</kbd>}</button>)}</nav>
      <div className="nav-label collections-label">COLLECTIONS <button className="tiny-plus" onClick={() => setShowAdd(true)} aria-label="Add collection"><Icon name="plus" size={14} /></button></div>
      {["All resources", "Favorites", "Work", "Web", "Apps"].map((item, index) => <button key={item} className={`collection ${category === item ? "selected" : ""}`} onClick={() => { setCategory(item); setSection("Dashboard"); }}><span className={`collection-dot d${index}`} />{item}<span className="collection-count">{item === "Favorites" ? resources.filter((r) => r.favorite).length : item === "All resources" ? resources.length : resources.filter((r) => r.category === item).length}</span></button>)}
      <div className="sidebar-bottom"><button className="nav-item" onClick={() => setDark(!dark)}><Icon name="moon" size={18} />{dark ? "Light appearance" : "Dark appearance"}</button><button className="nav-item"><Icon name="settings" size={18} />Settings</button><div className="sidebar-user"><div className="user-avatar">A</div><div><b>Alex Morgan</b><small>Local workspace</small></div><button className="icon-button" aria-label="More account options"><Icon name="more" /></button></div></div>
    </aside>

    <main className="main-area">
      <header className="topbar"><div className="breadcrumbs"><span>Workspace</span><Icon name="chevron" size={13} /><b>{section}</b></div><div className="top-actions"><button className="button quiet" onClick={() => setDark(!dark)} aria-label="Toggle theme"><Icon name="moon" size={16} /></button><button className="help-button" aria-label="Help">?</button><button className="profile-chip"><span className="user-avatar mini">A</span><span>Alex</span><Icon name="chevron" size={13} /></button></div></header>
      <div className="content-wrap">
        {section === "Analytics" ? <Analytics resources={resources} /> : section === "Explorer" ? <Explorer resources={filtered} onSelect={(r) => setSelectedId(r.id)} query={query} setQuery={setQuery} onAdd={() => setShowAdd(true)} /> : <>
          <div className="welcome-row"><div><h1>{section === "Quick Launch" ? "Quick Launch" : greeting()}</h1><p>{section === "Quick Launch" ? "Find a resource and open it in its default app." : "Everything you need, right where you left it."}</p></div><button className="button primary add-button" onClick={() => setShowAdd(true)}><Icon name="plus" size={17} /> Add resource</button></div>
          <div className={`search-box ${section === "Quick Launch" ? "command-search" : ""}`}><Icon name="search" size={19} /><input id="global-search" placeholder={section === "Quick Launch" ? "Search resources, aliases, tags…" : "Search your workspace"} value={query} onChange={(e) => setQuery(e.target.value)} /><span className="search-shortcut">{section === "Quick Launch" ? "ESC to close" : "⌘ K"}</span></div>
          <div className="section-heading"><div><h2>{query ? "Search results" : category === "Favorites" ? "Favorites" : "Pinned resources"}</h2><span>{filtered.length} {filtered.length === 1 ? "resource" : "resources"}</span></div><button className="text-button" onClick={() => { setSection("Explorer"); setCategory("All resources"); }}>View all <Icon name="arrow" size={15} /></button></div>
          {notice && <div className="notice" role="status"><span>{notice}</span><button className="icon-button" onClick={() => setNotice("")} aria-label="Dismiss"><Icon name="close" size={15} /></button></div>}
          {loading ? <div className="loading-state">Loading your workspace…</div> : filtered.length === 0 ? <div className="empty-state"><span className="empty-icon"><Icon name="folder" size={24} /></span><h3>{query ? "No matching resources" : "Your workspace is ready"}</h3><p>{query ? "Try another name, type, tag or location." : "Add a file, folder, app or secure web link to get started."}</p>{!query && <button className="button primary" onClick={() => setShowAdd(true)}><Icon name="plus" size={16} /> Add your first resource</button>}</div> : <div className="resource-grid" aria-label="Resources">
            {filtered.map((resource) => <article key={resource.id} style={{ gridColumn: `span ${cardWidths[resource.id] || 4}` }} className={`resource-card ${selectedId === resource.id ? "selected" : ""}`} draggable onDragStart={() => setDragId(resource.id)} onDragOver={(e) => e.preventDefault()} onDrop={() => reorder(resource.id)} onClick={() => setSelectedId(resource.id)} onDoubleClick={() => void launch(resource)}>
              <div className="card-top"><ResourceMark resource={resource} /><button className={`favorite-button ${resource.favorite ? "is-favorite" : ""}`} title={resource.favorite ? "Remove from favorites" : "Add to favorites"} aria-label={resource.favorite ? `Remove ${resource.displayName} from favorites` : `Add ${resource.displayName} to favorites`} onClick={(e) => { e.stopPropagation(); void update(resource.id, { favorite: !resource.favorite }); }}><Icon name="star" size={16} /></button></div>
              <div className="card-copy"><h3 title={resource.displayName}>{resource.displayName}</h3><p>{resource.description || resource.targets[0]?.canonicalUri || "No location set"}</p></div>
              <div className="card-footer"><span className="type-label">{resource.category || resource.resourceType}</span><button className="launch-button" aria-label={`Open ${resource.displayName}`} onClick={(e) => { e.stopPropagation(); void launch(resource); }}>Open <Icon name="arrow" size={13} /></button></div>
            </article>)}
            <button className="add-resource-card" onClick={() => setShowAdd(true)}><span><Icon name="plus" size={19} /></span><b>Add resource</b><small>File, folder, app or web link</small></button>
          </div>}
          <div className="section-heading recent-heading"><div><h2>Recently opened</h2><span>From this workspace</span></div><button className="text-button" onClick={() => setSection("Analytics")}>View activity <Icon name="arrow" size={15} /></button></div>
          <div className="recent-list">{resources.slice().sort((a, b) => (b.lastLaunchedUtc || b.updatedUtc).localeCompare(a.lastLaunchedUtc || a.updatedUtc)).slice(0, 4).map((resource) => <button key={resource.id} className="recent-row" onClick={() => void launch(resource)}><ResourceMark resource={resource} /><span className="recent-name"><b>{resource.displayName}</b><small>{resource.targets[0]?.canonicalUri}</small></span><span className="recent-time">{resource.lastLaunchedUtc ? relativeTime(resource.lastLaunchedUtc) : "Added recently"}</span><Icon name="arrow" size={15} /></button>)}</div>
        </>}
      </div>
    </main>
    <aside className="inspector"><div className="inspector-head"><h2>Inspector</h2><button className="icon-button" aria-label="More inspector options"><Icon name="more" /></button></div>
      {selected ? <><div className="inspector-resource"><ResourceMark resource={selected} /><div><h3>{selected.displayName}</h3><span>{selected.resourceType}</span></div></div><button className="button primary inspector-launch" onClick={() => void launch(selected)}><Icon name="arrow" size={16} /> Open resource</button><div className="inspector-section"><div className="inspector-label">LOCATION</div><div className="location-value">{selected.targets[0]?.canonicalUri || "No location available"}</div><button className="subtle-action" onClick={() => navigator.clipboard?.writeText(selected.targets[0]?.canonicalUri || "")}>Copy location</button></div><div className="inspector-section"><div className="inspector-label">CARD SIZE</div><div className="size-options" role="group" aria-label="Dashboard card size">{[{ value: 3, label: "Compact" }, { value: 4, label: "Standard" }, { value: 6, label: "Wide" }].map((item) => <button key={item.value} className={(cardWidths[selected.id] || 4) === item.value ? "chosen" : ""} onClick={() => resizeCard(selected.id, item.value)}>{item.label}</button>)}</div></div><div className="inspector-section"><div className="inspector-label">DETAILS</div><InfoRow label="Collection" value={selected.category || "Uncategorized"} /><InfoRow label="Launches" value={String(selected.launchCount || 0)} /><InfoRow label="Last opened" value={selected.lastLaunchedUtc ? relativeTime(selected.lastLaunchedUtc) : "No launch history"} /></div><div className="inspector-section"><div className="inspector-label">AVAILABILITY</div><div className="availability"><span className="availability-dot" /><span>Not checked</span><small>Metadata is collected in the desktop app</small></div><p className="provenance">File availability and metadata are inspected locally. No background drive scan.</p></div><div className="inspector-footer"><button className="subtle-action" onClick={() => { const name = window.prompt("Resource name", selected.displayName); if (name?.trim()) void update(selected.id, { displayName: name.trim() }); }}>Rename</button><button className="subtle-action danger" onClick={() => void remove(selected)}>Remove</button></div></> : <div className="inspector-empty"><span className="empty-icon"><Icon name="grid" /></span><p>Select a resource to inspect its details.</p></div>}
    </aside>
    {showAdd && <AddResourceDialog onClose={() => setShowAdd(false)} onCreate={(resource) => { setResources((all) => [...all, resource]); setSelectedId(resource.id); setNotice(`${resource.displayName} added to your workspace.`); }} />}
  </div>;
}

function greeting() { const hour = new Date().getHours(); return hour < 12 ? "Good morning, Alex" : hour < 18 ? "Good afternoon, Alex" : "Good evening, Alex"; }
function relativeTime(value: string) { const elapsed = Math.max(0, Date.now() - new Date(value).getTime()); const minutes = Math.floor(elapsed / 60000); if (minutes < 1) return "Just now"; if (minutes < 60) return `${minutes} min ago`; const hours = Math.floor(minutes / 60); return hours < 24 ? `${hours} hr ago` : `${Math.floor(hours / 24)} days ago`; }
function InfoRow({ label, value }: { label: string; value: string }) { return <div className="info-row"><span>{label}</span><b>{value}</b></div>; }
function Analytics({ resources }: { resources: Resource[] }) { const total = resources.reduce((sum, r) => sum + (r.launchCount || 0), 0); return <><div className="welcome-row"><div><h1>Activity</h1><p>Launch activity recorded by this workspace.</p></div></div><div className="analytics-summary"><div><span>Total launches</span><b>{total}</b></div><div><span>Resources</span><b>{resources.length}</b></div><div><span>Tracked by</span><b>Workspace</b></div></div><div className="section-heading"><div><h2>Most opened</h2><span>Local launch handoffs</span></div></div><div className="recent-list">{resources.slice().sort((a,b)=>(b.launchCount||0)-(a.launchCount||0)).map((r)=> <div className="recent-row" key={r.id}><ResourceMark resource={r}/><span className="recent-name"><b>{r.displayName}</b><small>Default app handoff count</small></span><b className="launch-count">{r.launchCount || 0}</b></div>)}</div><p className="analytics-note">Activity counts requests handled by Smart Workspace. A system handoff does not confirm that a document finished opening. External file usage is not observed.</p></> }
function Explorer({ resources, onSelect, query, setQuery, onAdd }: { resources: Resource[]; onSelect: (r: Resource) => void; query: string; setQuery: (q: string) => void; onAdd: () => void }) { return <><div className="welcome-row"><div><h1>Resource Explorer</h1><p>Browse and organize your workspace resources.</p></div><button className="button primary" onClick={onAdd}><Icon name="plus" size={16}/> Add resource</button></div><div className="search-box explorer-search"><Icon name="search" size={18}/><input placeholder="Search by name or location" value={query} onChange={(e)=>setQuery(e.target.value)}/></div><div className="table-shell"><table><thead><tr><th>NAME</th><th>TYPE</th><th>COLLECTION</th><th>LOCATION</th><th>LAUNCHES</th></tr></thead><tbody>{resources.map((r)=><tr key={r.id} onClick={()=>onSelect(r)} tabIndex={0} onKeyDown={(e)=>e.key==="Enter"&&onSelect(r)}><td><span className="table-name"><ResourceMark resource={r}/><b>{r.displayName}</b></span></td><td>{r.resourceType}</td><td>{r.category || "—"}</td><td className="table-location">{r.targets[0]?.canonicalUri}</td><td>{r.launchCount || 0}</td></tr>)}</tbody></table>{resources.length===0&&<div className="table-empty">No resources match your search.</div>}</div></> }
