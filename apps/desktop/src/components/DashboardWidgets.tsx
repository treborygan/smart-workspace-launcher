import { useEffect, useMemo, useState } from "react";
import type { FormEvent } from "react";
import type { DashboardWidget, ResourceRelationship, WidgetTemplate } from "@sw/contracts";
import type { Resource } from "../lib/contracts";
import { api } from "../lib/api";

type TemplateKind = WidgetTemplate["kind"];

interface Props {
  profileId: string;
  resources: Resource[];
  relationships: ResourceRelationship[];
  templates: WidgetTemplate[];
  widgets: DashboardWidget[];
  onChange: (templates: WidgetTemplate[], widgets: DashboardWidget[]) => void;
  onLaunch: (resource: Resource) => void;
  onInspect: (resourceId: string) => void;
  onNotice: (message: string) => void;
}

const kindLabels: Record<TemplateKind, string> = {
  resource_shortcuts: "Resource shortcuts",
  recent_launches: "Recent launches",
  launch_summary: "Launch summary",
  related_resources: "Related resources",
};

export function DashboardWidgets({ profileId, resources, relationships, templates, widgets, onChange, onLaunch, onInspect, onNotice }: Props) {
  const [name, setName] = useState("");
  const [kind, setKind] = useState<TemplateKind>("resource_shortcuts");
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [anchorId, setAnchorId] = useState("");
  const [limit, setLimit] = useState(5);
  const [period, setPeriod] = useState<"7d" | "30d" | "all">("7d");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);
  const resourceById = useMemo(() => new Map(resources.map((resource) => [resource.id, resource])), [resources]);
  const sortedWidgets = [...widgets].sort((a, b) => a.y - b.y || a.x - b.x || a.id.localeCompare(b.id));

  const resetForm = () => { setName(""); setSelectedIds([]); setAnchorId(resources[0]?.id || ""); setLimit(5); setPeriod("7d"); setEditingId(null); };
  const startEdit = (template: WidgetTemplate) => {
    setEditingId(template.id); setName(template.name); setKind(template.kind);
    if (template.kind === "resource_shortcuts") { setSelectedIds(template.config.resourceIds); setAnchorId(resources[0]?.id || ""); }
    if (template.kind === "related_resources") { setSelectedIds([]); setAnchorId(template.config.resourceId); setLimit(template.config.limit); }
    if (template.kind === "recent_launches") { setSelectedIds([]); setAnchorId(resources[0]?.id || ""); setLimit(template.config.limit); }
    if (template.kind === "launch_summary") { setSelectedIds([]); setAnchorId(resources[0]?.id || ""); setPeriod(template.config.period); }
    setExpanded(true);
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const safeName = name.trim();
    if (!safeName || safeName.length > 60) { onNotice("Widget template names must contain 1 to 60 characters."); return; }
    let template: WidgetTemplate;
    const id = editingId || crypto.randomUUID();
    if (kind === "resource_shortcuts") {
      const validIds = [...new Set(selectedIds)].filter((resourceId) => resourceById.has(resourceId)).slice(0, 12);
      if (!validIds.length) { onNotice("Choose at least one resource for this widget."); return; }
      template = { id, name: safeName, kind, config: { resourceIds: validIds } };
    } else if (kind === "related_resources") {
      if (!resourceById.has(anchorId)) { onNotice("Choose an anchor resource that belongs to this profile."); return; }
      template = { id, name: safeName, kind, config: { resourceId: anchorId, limit: Math.max(1, Math.min(12, limit)) } };
    } else if (kind === "recent_launches") {
      template = { id, name: safeName, kind, config: { limit: Math.max(1, Math.min(20, limit)) } };
    } else {
      template = { id, name: safeName, kind, config: { period } };
    }
    const nextTemplates = templates.some((item) => item.id === id)
      ? templates.map((item) => item.id === id ? template : item)
      : [...templates, template];
    if (nextTemplates.length > 20) { onNotice("A profile can have up to 20 widget templates."); return; }
    onChange(nextTemplates, widgets);
    resetForm(); setExpanded(false);
  };
  const addInstance = (template: WidgetTemplate) => {
    if (widgets.length >= 50) { onNotice("A dashboard can have up to 50 widget instances."); return; }
    const nextY = widgets.reduce((max, widget) => Math.max(max, widget.y + widget.h), 0);
    onChange(templates, [...widgets, { id: crypto.randomUUID(), templateId: template.id, x: 0, y: nextY, w: 4, h: 2 }]);
  };
  const deleteTemplate = (id: string) => {
    const nextTemplates = templates.filter((template) => template.id !== id);
    const nextWidgets = widgets.filter((widget) => widget.templateId !== id);
    onChange(nextTemplates, nextWidgets);
    if (editingId === id) resetForm();
  };
  const patchWidget = (widgetId: string, patch: Partial<DashboardWidget>) => {
    onChange(templates, widgets.map((widget) => widget.id === widgetId ? { ...widget, ...patch } : widget));
  };
  const moveWidget = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    if (target < 0 || target >= sortedWidgets.length) return;
    const moved = [...sortedWidgets];
    [moved[index], moved[target]] = [moved[target], moved[index]];
    onChange(templates, moved.map((widget, position) => ({ ...widget, x: 0, y: position * 2 })));
  };

  return <section className="dashboard-widgets" aria-labelledby="dashboard-widgets-title">
    <header className="dashboard-widgets-header"><div><h2 id="dashboard-widgets-title">Dashboard widgets</h2><p>Reusable, profile-local views. Activity reflects launches recorded by this app only.</p></div><button className="button secondary" aria-expanded={expanded} onClick={() => { if (!expanded) { resetForm(); setExpanded(true); } else { resetForm(); setExpanded(false); } }}>{expanded ? "Close template editor" : "New template"}</button></header>
    {expanded && <form className="widget-template-form" onSubmit={submit}>
      <label>Template name<input autoComplete="off" maxLength={60} value={name} onChange={(event) => setName(event.target.value)} placeholder="e.g. Project starters" required /></label>
      <label>View type<select value={kind} onChange={(event) => { setKind(event.target.value as TemplateKind); setSelectedIds([]); }}>
        {(Object.keys(kindLabels) as TemplateKind[]).map((value) => <option key={value} value={value}>{kindLabels[value]}</option>)}
      </select></label>
      {kind === "resource_shortcuts" && <fieldset className="widget-resource-picker"><legend>Resources · choose up to 12</legend>{resources.length ? resources.map((resource) => <label key={resource.id}><input type="checkbox" checked={selectedIds.includes(resource.id)} disabled={!selectedIds.includes(resource.id) && selectedIds.length >= 12} onChange={(event) => setSelectedIds((current) => event.target.checked ? [...current, resource.id] : current.filter((id) => id !== resource.id))} /><span>{resource.displayName}</span></label>) : <p className="widget-muted">Add a resource before creating a shortcuts widget.</p>}</fieldset>}
      {kind === "related_resources" && <label>Anchor resource<select value={anchorId} onChange={(event) => setAnchorId(event.target.value)}>{resources.map((resource) => <option key={resource.id} value={resource.id}>{resource.displayName}</option>)}</select></label>}
      {(kind === "recent_launches" || kind === "related_resources") && <label>Items to show<input type="number" min={1} max={kind === "recent_launches" ? 20 : 12} value={limit} onChange={(event) => setLimit(Number(event.target.value))} /></label>}
      {kind === "launch_summary" && <label>History period<select value={period} onChange={(event) => setPeriod(event.target.value as typeof period)}><option value="7d">Last 7 days</option><option value="30d">Last 30 days</option><option value="all">All retained history</option></select></label>}
      {templates.length >= 20 && !editingId && <p className="widget-warning" role="status">This profile has reached the 20-template limit.</p>}
      <footer><button type="button" className="button secondary" onClick={() => { resetForm(); setExpanded(false); }}>Cancel</button><button className="button primary" disabled={templates.length >= 20 && !editingId}>{editingId ? "Save template" : "Create template"}</button></footer>
    </form>}
    {templates.length > 0 && <div className="widget-template-list" aria-label="Saved widget templates">{templates.map((template) => <article className="widget-template-row" key={template.id}><div><b>{template.name}</b><small>{kindLabels[template.kind]}</small></div><button className="button secondary" onClick={() => addInstance(template)} disabled={widgets.length >= 50}>Add widget</button><button className="text-button" onClick={() => startEdit(template)}>Edit</button><button className="text-button danger" onClick={() => deleteTemplate(template.id)} aria-label={`Delete ${template.name} template`}>Delete</button></article>)}</div>}
    {sortedWidgets.length > 0 ? <div className="dashboard-widget-grid" aria-label="Placed dashboard widgets">{sortedWidgets.map((widget, index) => {
      const template = templates.find((item) => item.id === widget.templateId);
      if (!template) return <div key={widget.id} className="dashboard-widget-card widget-error" role="status">This widget template is missing. Remove this instance or restore the profile archive.</div>;
      const x = Math.max(0, Math.min(11, widget.x)); const width = Math.max(1, Math.min(12 - x, widget.w)); const height = Math.max(1, Math.min(4, widget.h));
      return <article key={widget.id} className="dashboard-widget-card" style={{ gridColumn: `${x + 1} / span ${width}`, gridRow: `${Math.max(1, widget.y + 1)} / span ${height}`, order: widget.y * 12 + x }}>
        <header><div><h3>{template.name}</h3><small>{kindLabels[template.kind]}</small></div><div className="widget-controls"><button className="move-card" aria-label={`Move ${template.name} earlier`} disabled={index === 0} onClick={() => moveWidget(index, -1)}>↑</button><button className="move-card" aria-label={`Move ${template.name} later`} disabled={index === sortedWidgets.length - 1} onClick={() => moveWidget(index, 1)}>↓</button><button className="move-card" aria-label={`Move ${template.name} left`} disabled={x <= 0} onClick={() => patchWidget(widget.id, { x: Math.max(0, x - 1) })}>←</button><button className="move-card" aria-label={`Move ${template.name} right`} disabled={x + width >= 12} onClick={() => patchWidget(widget.id, { x: Math.min(12 - width, x + 1) })}>→</button><button className="move-card" aria-label={`Make ${template.name} narrower`} disabled={width <= 1} onClick={() => patchWidget(widget.id, { w: Math.max(1, width - 1) })}>−</button><button className="move-card" aria-label={`Make ${template.name} wider`} disabled={width >= 12 - x} onClick={() => patchWidget(widget.id, { w: Math.min(12 - x, width + 1) })}>+</button><button className="move-card" aria-label={`Make ${template.name} shorter`} disabled={height <= 1} onClick={() => patchWidget(widget.id, { h: Math.max(1, height - 1) })}>⌃</button><button className="move-card" aria-label={`Make ${template.name} taller`} disabled={height >= 4} onClick={() => patchWidget(widget.id, { h: Math.min(4, height + 1) })}>⌄</button><button className="text-button danger" aria-label={`Remove ${template.name} widget`} onClick={() => onChange(templates, widgets.filter((item) => item.id !== widget.id))}>Remove</button></div></header>
        <WidgetContent key={`${widget.id}:${template.id}`} profileId={profileId} template={template} resources={resourceById} relationships={relationships} onLaunch={onLaunch} onInspect={onInspect} onNotice={onNotice} />
      </article>;
    })}</div> : templates.length === 0 ? <p className="widget-empty">Create a template, then add it to the dashboard. No code or scripts can be stored in a template.</p> : <p className="widget-empty">Choose “Add widget” beside a template to place it on the dashboard.</p>}
  </section>;
}

function WidgetResourceMark({ resource }: { resource: Resource }) {
  const symbol = resource.resourceType === "folder" ? "▰" : resource.resourceType === "url" ? "↗" : resource.resourceType === "app" ? "▣" : resource.displayName.split(".").pop()?.slice(0, 2).toUpperCase() || "•";
  return <span className={`resource-mark ${resource.resourceType === "folder" ? "folder" : resource.resourceType === "url" ? "web" : ""}`} aria-hidden="true">{symbol}</span>;
}

function WidgetContent({ profileId, template, resources, relationships, onLaunch, onInspect, onNotice }: {
  profileId: string; template: WidgetTemplate; resources: Map<string, Resource>; relationships: ResourceRelationship[];
  onLaunch: (resource: Resource) => void; onInspect: (resourceId: string) => void; onNotice: (message: string) => void;
}) {
  const [events, setEvents] = useState<Awaited<ReturnType<typeof api.analyticsEvents>>>([]);
  const [total, setTotal] = useState(0);
  const [recording, setRecording] = useState<boolean | null>(null);
  const [error, setError] = useState("");
  const historyKind = template.kind === "recent_launches" || template.kind === "launch_summary";
  const period = template.kind === "launch_summary" ? template.config.period : "all";
  useEffect(() => {
    if (!historyKind) return;
    let alive = true;
    const query = template.kind === "launch_summary" && period !== "all"
      ? { fromUtc: new Date(Date.now() - (period === "7d" ? 7 : 30) * 86400000).toISOString() }
      : {};
    void Promise.all([api.usageSettings(profileId), api.analyticsEvents({ profileId, ...query, limit: template.kind === "recent_launches" ? template.config.limit : 100, offset: 0 }), api.analyticsSummary(profileId, query)])
      .then(([settings, rows, summary]) => { if (alive) { setRecording(settings.recordingEnabled); setEvents(rows); setTotal(summary.totalLaunches); setError(""); } })
      .catch((reason: unknown) => { if (alive) setError(reason instanceof Error ? reason.message : "Recorded launch history is unavailable."); });
    return () => { alive = false; };
  }, [profileId, template, historyKind, period]);

  if (error) return <p className="widget-empty widget-error" role="status">Could not load locally recorded activity: {error}</p>;
  if (template.kind === "resource_shortcuts") {
    const items = template.config.resourceIds.map((id) => resources.get(id));
    if (!items.length || items.every((resource) => !resource)) return <p className="widget-empty">No linked resources remain in this profile. Edit the template to choose available resources.</p>;
    return <div className="widget-shortcuts">{items.map((resource, index) => resource ? <button key={resource.id} className="widget-resource" onClick={() => onLaunch(resource)}><WidgetResourceMark resource={resource} /><span><b>{resource.displayName}</b><small>{resource.resourceType}</small></span><span>Open</span></button> : <p className="widget-empty" key={`missing-${index}`}>A linked resource is no longer in this profile.</p>)}</div>;
  }
  if (template.kind === "related_resources") {
    const anchor = resources.get(template.config.resourceId);
    if (!anchor) return <p className="widget-empty">The anchor resource is no longer in this profile. Edit this template to choose another resource.</p>;
    const relatedIds = relationships.filter((edge) => edge.sourceResourceId === anchor.id || edge.targetResourceId === anchor.id).map((edge) => edge.sourceResourceId === anchor.id ? edge.targetResourceId : edge.sourceResourceId).slice(0, template.config.limit);
    return <div className="widget-shortcuts"><p className="widget-muted">Related to <button className="widget-link" onClick={() => onInspect(anchor.id)}>{anchor.displayName}</button></p>{relatedIds.length ? relatedIds.map((id) => { const resource = resources.get(id); return resource ? <button key={id} className="widget-resource" onClick={() => onLaunch(resource)}><WidgetResourceMark resource={resource} /><span><b>{resource.displayName}</b><small>Related resource</small></span><span>Open</span></button> : null; }) : <p className="widget-empty">No related resources have been linked yet.</p>}</div>;
  }
  if (template.kind === "launch_summary") return <div className="widget-summary"><strong>{total}</strong><span>recorded launches</span><small>{recording === false ? "Recording is off; this is retained history." : "Recorded by Smart Workspace Launcher."}</small></div>;
  const visible = events.slice(0, template.config.limit);
  return <div className="widget-shortcuts">{recording === false && <p className="widget-muted">Recording is off. Previously retained events are shown.</p>}{visible.length ? visible.map((event) => { const resource = resources.get(event.resourceId); return <button key={event.requestId} className="widget-resource" onClick={() => resource ? onLaunch(resource) : onNotice("This launch refers to a resource no longer in the profile.")}><span><b>{resource?.displayName || "Removed resource"}</b><small>{new Date(event.launchedAtUtc).toLocaleString()} · {event.status === "handed_off" ? "handed off" : event.status}</small></span><span>Open</span></button>; }) : <p className="widget-empty">No recorded launches in this profile yet. Only app-recorded events appear here.</p>}</div>;
}
