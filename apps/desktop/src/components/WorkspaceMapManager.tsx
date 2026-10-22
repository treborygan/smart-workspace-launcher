import { useMemo, useState } from "react";
import type { ResourceRelationship, WorkspaceSnapshot, WorkspaceSnapshotRestoreResult } from "@sw/contracts";
import type { Resource } from "../lib/contracts";

type Props = {
  profileId: string;
  resources: Resource[];
  relationships: ResourceRelationship[];
  snapshots: WorkspaceSnapshot[];
  onClose: () => void;
  onSelectResource: (id: string) => void;
  onSaveRelationship: (relationship: ResourceRelationship) => Promise<ResourceRelationship>;
  onDeleteRelationship: (id: string) => Promise<void>;
  onCreateSnapshot: (name: string) => Promise<WorkspaceSnapshot>;
  onDeleteSnapshot: (id: string) => Promise<void>;
  onRestoreSnapshot: (id: string) => Promise<WorkspaceSnapshotRestoreResult>;
};

const colors = ["#4f78d1", "#ca7a43", "#3e9b7a", "#9a68bf", "#b45c73", "#65859b"];
const MAX_VISIBLE_GRAPH_NODES = 17;

export function WorkspaceMapManager(props: Props) {
  const { profileId, resources, relationships, snapshots } = props;
  const [sourceId, setSourceId] = useState("");
  const [targetId, setTargetId] = useState("");
  const [label, setLabel] = useState("");
  const [snapshotName, setSnapshotName] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const sortedResources = useMemo(() => [...resources].sort((a, b) => a.displayName.localeCompare(b.displayName) || a.id.localeCompare(b.id)), [resources]);
  const focusId = selectedId && resources.some((resource) => resource.id === selectedId) ? selectedId : sortedResources[0]?.id ?? "";
  const nodes = useMemo(() => {
    let ordered = sortedResources;
    if (resources.length > MAX_VISIBLE_GRAPH_NODES) {
      const neighbors = relationships.filter((edge) => edge.sourceResourceId === focusId || edge.targetResourceId === focusId)
        .map((edge) => edge.sourceResourceId === focusId ? edge.targetResourceId : edge.sourceResourceId)
        .filter((id, index, all) => id !== focusId && all.indexOf(id) === index)
        .sort((a, b) => a.localeCompare(b));
      const allowed = new Set([focusId, ...neighbors.slice(0, MAX_VISIBLE_GRAPH_NODES - 1)]);
      ordered = sortedResources.filter((resource) => allowed.has(resource.id));
    }
    const radius = Math.max(95, Math.min(190, 24 * ordered.length));
    return new Map(ordered.map((resource, index) => {
      const angle = -Math.PI / 2 + (Math.PI * 2 * index) / Math.max(ordered.length, 1);
      return [resource.id, { resource, x: 250 + radius * Math.cos(angle), y: 220 + radius * Math.sin(angle) }];
    }));
  }, [focusId, relationships, resources.length, sortedResources]);
  const selected = selectedId ? nodes.get(selectedId)?.resource : undefined;
  const related = selected ? relationships.filter((edge) => edge.sourceResourceId === selected.id || edge.targetResourceId === selected.id).map((edge) => ({ edge, id: edge.sourceResourceId === selected.id ? edge.targetResourceId : edge.sourceResourceId })).filter(({ id }) => nodes.has(id)) : [];
  const submitRelationship = async () => {
    setError(""); setNotice("");
    if (!sourceId || !targetId || !label.trim()) { setError("Choose two resources and add a label."); return; }
    if (sourceId === targetId) { setError("A resource cannot be linked to itself."); return; }
    setBusy(true);
    try {
      await props.onSaveRelationship({ id: crypto.randomUUID(), profileId, sourceResourceId: sourceId, targetResourceId: targetId, label: label.trim() });
      setLabel(""); setNotice("Resource link saved.");
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Could not save this resource link."); }
    finally { setBusy(false); }
  };
  const createSnapshot = async () => {
    setError(""); setNotice("");
    if (!snapshotName.trim()) { setError("Enter a name for this snapshot."); return; }
    setBusy(true);
    try { await props.onCreateSnapshot(snapshotName.trim()); setSnapshotName(""); setNotice("Workspace snapshot created."); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Could not create this snapshot."); }
    finally { setBusy(false); }
  };
  const restore = async (id: string) => {
    setError(""); setNotice(""); setBusy(true);
    try {
      const result = await props.onRestoreSnapshot(id);
      setNotice(result.missingResourceIds.length ? `Layouts restored. ${result.missingResourceIds.length} saved resource reference(s) are no longer in this profile.` : "Workspace layouts restored.");
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Could not restore this snapshot."); }
    finally { setBusy(false); }
  };
  const removeRelationship = async (id: string) => {
    setError(""); setNotice(""); setBusy(true);
    try { await props.onDeleteRelationship(id); setNotice("Resource link removed."); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Could not remove this resource link."); }
    finally { setBusy(false); }
  };
  const removeSnapshot = async (id: string) => {
    setError(""); setNotice(""); setBusy(true);
    try { await props.onDeleteSnapshot(id); setNotice("Workspace snapshot deleted."); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Could not delete this snapshot."); }
    finally { setBusy(false); }
  };

  return <section className="workspace-map-manager">
    <div className="welcome-row"><div><h1>Related resources</h1><p>Connect resources that belong together, then save named checkpoints of your dashboard and Dock layouts.</p></div><button className="button secondary" onClick={props.onClose}>Back to workspace</button></div>
    {error && <p className="form-error" role="alert">{error}</p>}{notice && <p className="workspace-map-notice" role="status">{notice}</p>}
    <div className="workspace-map-layout">
      <section className="workspace-map-card" aria-label="Related resource graph">
        <header><div><h2>Resource graph</h2><p>{resources.length} resources · {relationships.length} links</p></div>{resources.length > MAX_VISIBLE_GRAPH_NODES && <label className="workspace-graph-focus">Focus resource<select aria-label="Focus resource" value={focusId} onChange={(event) => setSelectedId(event.target.value)}>{sortedResources.map((resource) => <option key={resource.id} value={resource.id}>{resource.displayName}</option>)}</select></label>}</header>
        {resources.length > MAX_VISIBLE_GRAPH_NODES && <p className="workspace-graph-limit">Showing the selected resource and up to {MAX_VISIBLE_GRAPH_NODES - 1} directly related resources for readability.</p>}
        {resources.length === 0 ? <p className="empty-state">Add resources to this profile to build a relationship graph.</p> : <div className="workspace-graph-scroll"><svg className="workspace-graph" viewBox="0 0 500 440" role="group" aria-label="Related resource graph">
          {relationships.map((edge) => { const from = nodes.get(edge.sourceResourceId); const to = nodes.get(edge.targetResourceId); if (!from || !to) return null; const x = (from.x + to.x) / 2; const y = (from.y + to.y) / 2; return <g key={edge.id} className="workspace-graph-edge"><line x1={from.x} y1={from.y} x2={to.x} y2={to.y} /><rect x={x - Math.min(42, edge.label.length * 3.4)} y={y - 10} width={Math.min(84, edge.label.length * 6.8)} height="19" rx="8" /><text x={x} y={y + 3}>{edge.label.slice(0, 14)}</text></g>; })}
          {[...nodes.values()].map(({ resource, x, y }, index) => <g key={resource.id} className={`workspace-graph-node ${resource.id === selectedId ? "selected" : ""}`} role="button" tabIndex={0} aria-label={`Select ${resource.displayName}`} onClick={() => setSelectedId(resource.id)} onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); setSelectedId(resource.id); } }}>
            <circle cx={x} cy={y} r="25" style={{ fill: colors[index % colors.length] }} /><text className="workspace-graph-mark" x={x} y={y + 4}>{resource.displayName.slice(0, 1).toUpperCase()}</text><text className="workspace-graph-name" x={x} y={y + 43}>{resource.displayName.slice(0, 20)}</text>
          </g>)}
        </svg></div>}
        {selected && <aside className="workspace-graph-inspector"><div><b>{selected.displayName}</b><small>{related.length} related {related.length === 1 ? "resource" : "resources"}</small></div><button className="button secondary" onClick={() => props.onSelectResource(selected.id)}>Open in Explorer</button>{related.length > 0 && <ul>{related.map(({ edge, id }) => <li key={edge.id}><button className="workspace-map-link" onClick={() => setSelectedId(id)}>{nodes.get(id)?.resource.displayName}</button><span>{edge.label}</span><button className="icon-button" aria-label={`Remove ${edge.label} link`} disabled={busy} onClick={() => void removeRelationship(edge.id)}>×</button></li>)}</ul>}</aside>}
      <div className="workspace-map-create-link"><h3>Add a resource link</h3><label>First resource<select aria-label="First resource" value={sourceId} onChange={(event) => setSourceId(event.target.value)}><option value="">Choose resource</option>{resources.map((resource) => <option key={resource.id} value={resource.id}>{resource.displayName}</option>)}</select></label><label>Related resource<select aria-label="Related resource" value={targetId} onChange={(event) => setTargetId(event.target.value)}><option value="">Choose resource</option>{resources.map((resource) => <option key={resource.id} value={resource.id}>{resource.displayName}</option>)}</select></label><label>Relationship label<input aria-label="Relationship label" maxLength={80} value={label} onChange={(event) => setLabel(event.target.value)} placeholder="e.g. supports" /></label><button className="button primary" disabled={busy || resources.length < 2} onClick={() => void submitRelationship()}>Add link</button></div>
      </section>
      <section className="workspace-map-card workspace-snapshot-card" aria-label="Workspace snapshots"><header><div><h2>Workspace snapshots</h2><p>Save or restore dashboard and Dock arrangements.</p></div></header><div className="workspace-snapshot-create"><label>Snapshot name<input aria-label="Snapshot name" maxLength={100} value={snapshotName} onChange={(event) => setSnapshotName(event.target.value)} placeholder="e.g. Weekly planning" /></label><button className="button primary" disabled={busy} onClick={() => void createSnapshot()}>Save snapshot</button></div>
        {snapshots.length === 0 ? <p className="empty-state">No snapshots yet. Save your current workspace layouts to create one.</p> : <ul className="workspace-snapshot-list">{[...snapshots].sort((a, b) => b.createdUtc.localeCompare(a.createdUtc)).map((snapshot) => <li key={snapshot.id}><div><b>{snapshot.name}</b><small>{new Date(snapshot.createdUtc).toLocaleString()} · {snapshot.resourceIds.length} resources</small></div><button className="button secondary" disabled={busy} onClick={() => void restore(snapshot.id)}>Restore</button><button className="icon-button" aria-label={`Delete ${snapshot.name}`} disabled={busy} onClick={() => void removeSnapshot(snapshot.id)}>×</button></li>)}</ul>}
        <p className="workspace-snapshot-note">Restoring a checkpoint updates layouts only. It does not remove resources or open them.</p>
      </section>
    </div>
  </section>;
}
