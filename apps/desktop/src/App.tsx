import { useEffect, useMemo, useState } from "react";
import type { FormEvent } from "react";
import { Icon } from "./components/Icon";
import { QuickLaunchOverlay } from "./components/QuickLaunchOverlay";
import { DockWindow } from "./components/DockWindow";
import { ResourceMetadataPanel } from "./components/ResourceMetadataPanel";
import { AnalyticsView } from "./components/AnalyticsView";
import { ProfileArchiveControls } from "./components/ProfileArchiveControls";
import { ResourceExplorer } from "./components/ResourceExplorer";
import { ResourceEditorForm } from "./components/ResourceEditorForm";
import { ProfileThemeSettings } from "./components/ProfileThemeSettings";
import { SmartCollectionsManager } from "./components/SmartCollectionsManager";
import { DynamicShortcutsManager } from "./components/DynamicShortcutsManager";
import { WorkspaceSequencesManager } from "./components/WorkspaceSequencesManager";
import { WorkspaceMapManager } from "./components/WorkspaceMapManager";
import { ResourceImportAssistant } from "./components/ResourceImportAssistant";
import { DashboardWidgets } from "./components/DashboardWidgets";
import { DevicePathSettings } from "./components/DevicePathSettings";
import { ExtensionsRegistry } from "./components/ExtensionsRegistry";
import { api } from "./lib/api";
import type { DashboardLayout, DashboardWidget, DynamicShortcut, QuickLaunchHotkeyStatus, ResourcePatch, ResourceRelationship, ResourceRelocationPreview, ResourceRelocationPreviewRequest, SmartCollection, WidgetTemplate, WorkspaceSequence, WorkspaceSnapshot } from "@sw/contracts";
import type { Profile, Resource } from "./lib/contracts";
import { matchesSmartCollection } from "./lib/smartCollections";
import { DEFAULT_THEME, resolveAppearance, themeForProfile } from "./lib/profileTheme";
import "./styles.css";

type Section = "Dashboard" | "Quick Launch" | "Explorer" | "Analytics" | "Dock" | "Smart Collections" | "Quick Launch shortcuts" | "Workspace sequences" | "Resource graph & snapshots" | "Import resources" | "Device paths" | "Extensions";
type ShortcutDraft = { value: string; dirty: boolean };
const nav = [
  { label: "Dashboard" as const, icon: "grid" as const },
  { label: "Quick Launch" as const, icon: "search" as const },
  { label: "Explorer" as const, icon: "folder" as const },
  { label: "Analytics" as const, icon: "chart" as const },
  { label: "Dock" as const, icon: "grid" as const },
  { label: "Smart Collections" as const, icon: "grid" as const },
  { label: "Quick Launch shortcuts" as const, icon: "search" as const },
  { label: "Workspace sequences" as const, icon: "grid" as const },
  { label: "Resource graph & snapshots" as const, icon: "grid" as const },
  { label: "Import resources" as const, icon: "folder" as const },
  { label: "Device paths" as const, icon: "folder" as const },
  { label: "Extensions" as const, icon: "grid" as const },
];

function ResourceMark({ resource }: { resource: Resource }) {
  const ext = resource.displayName.split(".").pop()?.toLowerCase();
  const mark = resource.resourceType === "folder" ? "▱" : resource.resourceType === "url" ? "◎" : ext === "xlsx" ? "X" : ext === "pptx" ? "P" : resource.displayName.slice(0, 1).toUpperCase();
  const color = resource.resourceType === "folder" ? "folder" : ext === "xlsx" ? "excel" : resource.resourceType === "url" ? "web" : "app";
  return <span className={`resource-mark ${color}`} aria-hidden="true">{mark}</span>;
}

function AddResourceDialog({ profileId, onClose, onCreate }: { profileId?: string; onClose: () => void; onCreate: (resource: Resource) => void }) {
  const [name, setName] = useState("");
  const [uri, setUri] = useState("");
  const [kind, setKind] = useState<"file" | "folder" | "app">("file");
  const [error, setError] = useState("");
  const submit = async (event: FormEvent) => {
    event.preventDefault(); setError("");
    if (!name.trim() || !uri.trim()) { setError("Add a name and a file path or HTTPS address."); return; }
    if (/^https?:\/\//i.test(uri) && !/^https:\/\//i.test(uri)) { setError("Only HTTPS addresses are supported."); return; }
    try {
      const resource = await api.create({ profileId, resourceType: /^https:\/\//i.test(uri) ? "url" : kind, displayName: name.trim(), defaultAction: "open", targets: [{ id: crypto.randomUUID(), kind: /^https:\/\//i.test(uri) ? "url" : kind, canonicalUri: uri.trim(), priority: 0, arguments: [], active: true }] });
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
  const [smartCollections, setSmartCollections] = useState<SmartCollection[]>([]);
  const [dynamicShortcuts, setDynamicShortcuts] = useState<DynamicShortcut[]>([]);
  const [workspaceSequences, setWorkspaceSequences] = useState<WorkspaceSequence[]>([]);
  const [resourceRelationships, setResourceRelationships] = useState<ResourceRelationship[]>([]);
  const [workspaceSnapshots, setWorkspaceSnapshots] = useState<WorkspaceSnapshot[]>([]);
  const [widgetTemplates, setWidgetTemplates] = useState<WidgetTemplate[]>([]);
  const [dashboardWidgets, setDashboardWidgets] = useState<DashboardWidget[]>([]);
  const [selectedSmartCollections, setSelectedSmartCollections] = useState<Record<string, string | null>>({});
  const [activeProfileId, setActiveProfileId] = useState(() => localStorage.getItem("swl-active-profile") || "default");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editingResource, setEditingResource] = useState<string | null>(null);
  const [section, setSection] = useState<Section>("Dashboard");
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("All resources");
  const [showAdd, setShowAdd] = useState(false);
  const [notice, setNotice] = useState("");
  const [loading, setLoading] = useState(true);
  const [dragId, setDragId] = useState<string | null>(null);
  const [cardWidths, setCardWidths] = useState<Record<string, number>>({});
  const [cardHeights, setCardHeights] = useState<Record<string, number>>({});
  const [gridColumns, setGridColumns] = useState(() => columnsForViewport(window.innerWidth));
  const [dockLayout, setDockLayout] = useState<DashboardLayout | null>(null);
  const [themePreference, setThemePreference] = useState(DEFAULT_THEME);
  const [quickLaunchOpen, setQuickLaunchOpen] = useState(false);
  const [hotkeyStatus, setHotkeyStatus] = useState<QuickLaunchHotkeyStatus | null>(null);
  const [nativeWindowLabel, setNativeWindowLabel] = useState<string | null>(null);
  const [quickLaunchSignal, setQuickLaunchSignal] = useState(0);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [shortcutDrafts, setShortcutDrafts] = useState<Record<string, ShortcutDraft>>({});
  const [shortcutSaving, setShortcutSaving] = useState(false);
  const selectedProfile = profiles.find((profile) => profile.id === activeProfileId);
  let configuredShortcut = "Alt+Space";
  try { configuredShortcut = JSON.parse(selectedProfile?.settingsJson || "{}").quickLaunchShortcut || "Alt+Space"; } catch { /* keep the default */ }
  const shortcutDraft = shortcutDrafts[activeProfileId]?.value ?? configuredShortcut;

  const loadProfile = async (preferredProfileId?: string, activateProfile = false) => {
    const availableProfiles = await api.profiles();
    const rememberedProfile = preferredProfileId || localStorage.getItem("swl-active-profile");
    const profile = availableProfiles.find((item) => item.id === rememberedProfile) || availableProfiles[0];
    if (profile && activateProfile) await api.switchProfile(profile.id);
    if (profile) {
      let configured = "Alt+Space";
      try { configured = JSON.parse(profile.settingsJson || "{}").quickLaunchShortcut || "Alt+Space"; } catch { /* keep the default */ }
      setShortcutDrafts((current) => current[profile.id] ? current : { ...current, [profile.id]: { value: configured, dirty: false } });
      setThemePreference(themeForProfile(profile));
    }
    const [items, layout, dock, collections, shortcuts, sequences, relationships, snapshots] = await Promise.all([api.resources(profile?.id), profile ? api.layout(profile.id) : Promise.resolve(null), profile ? api.layout(profile.id, "dock") : Promise.resolve(null), profile ? api.smartCollections(profile.id) : Promise.resolve([]), profile ? api.dynamicShortcuts(profile.id) : Promise.resolve([]), profile ? api.workspaceSequences(profile.id) : Promise.resolve([]), profile ? api.resourceRelationships(profile.id) : Promise.resolve([]), profile ? api.workspaceSnapshots(profile.id) : Promise.resolve([])]);
    const restored = restoreLayout(items, layout);
    setProfiles(availableProfiles); setActiveProfileId(profile?.id || ""); setResources(restored.resources); setCardWidths(restored.widths); setCardHeights(restored.heights); setDockLayout(dock); setSmartCollections(collections); setDynamicShortcuts(shortcuts); setWorkspaceSequences(sequences); setResourceRelationships(relationships); setWorkspaceSnapshots(snapshots); setWidgetTemplates(layout?.widgetTemplates || []); setDashboardWidgets(layout?.widgets || []);
    if (profile && selectedSmartCollections[profile.id] && !collections.some((item) => item.id === selectedSmartCollections[profile.id])) setSelectedSmartCollections((current) => ({ ...current, [profile.id]: null }));
    if (profile) localStorage.setItem("swl-active-profile", profile.id);
    setSelectedId((old) => old && items.some((r) => r.id === old) ? old : restored.resources[0]?.id ?? null);
    return profile?.id || "";
  };
  const reload = async () => {
    try { await loadProfile(undefined, true); }
    catch (e) { setNotice(e instanceof Error ? e.message : "Could not connect to the local workspace."); }
    finally { setLoading(false); }
  };
  useEffect(() => { void reload(); }, []);
  useEffect(() => {
    const resize = () => setGridColumns(columnsForViewport(window.innerWidth));
    window.addEventListener("resize", resize); return () => window.removeEventListener("resize", resize);
  }, []);
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) { setNativeWindowLabel("browser"); return; }
    let cancelled = false;
    void import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
      if (cancelled) return;
      const label = getCurrentWindow().label;
      setNativeWindowLabel(label);
      if (label === "quick-launch") setQuickLaunchOpen(true);
    });
    return () => { cancelled = true; };
  }, []);
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      const browserHotkey = event.altKey && event.code === "Space" && !("__TAURI_INTERNALS__" in window);
      if (browserHotkey || ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k")) {
        event.preventDefault(); setQuickLaunchOpen(true);
      }
      if (event.key === "Escape") { setShowAdd(false); setQuickLaunchOpen(false); setQuery(""); }
    };
    window.addEventListener("keydown", handler); return () => window.removeEventListener("keydown", handler);
  }, []);
  useEffect(() => {
    let disposed = false;
    let unlistenToggle: () => void = () => {};
    let unlistenStatus: () => void = () => {};
    const toggleListener = nativeWindowLabel === "quick-launch"
      ? api.onQuickLaunchToggle(() => {
        setQuickLaunchOpen(true); setQuickLaunchSignal((value) => value + 1);
        void loadProfile().catch((e: unknown) => setNotice(e instanceof Error ? e.message : "Could not refresh the active profile."));
      })
      : Promise.resolve(() => undefined);
    const syncShortcutStatus = (statusValue: QuickLaunchHotkeyStatus) => {
      setHotkeyStatus(statusValue);
      if (activeProfileId) setShortcutDrafts((current) => current[activeProfileId]?.dirty ? current : { ...current, [activeProfileId]: { value: statusValue.shortcut, dirty: false } });
    };
    Promise.all([toggleListener, api.onQuickLaunchHotkeyStatus(syncShortcutStatus)]).then(([toggle, status]) => {
      if (disposed) { toggle(); status(); return; }
      unlistenToggle = toggle; unlistenStatus = status;
      void api.quickLaunchHotkeyStatus().then(syncShortcutStatus).catch(() => undefined);
    }).catch((e: unknown) => setNotice(e instanceof Error ? `Quick Launch hotkey status unavailable: ${e.message}` : "Quick Launch hotkey status unavailable."));
    return () => { disposed = true; unlistenToggle(); unlistenStatus(); };
  }, [nativeWindowLabel, activeProfileId]);
  useEffect(() => {
    if (nativeWindowLabel !== "dock") return;
    let disposed = false;
    let unlisten: () => void = () => {};
    void api.onWorkspaceStateChanged(({ profileId }) => {
      if (profileId === activeProfileId || profiles.some((profile) => profile.id === profileId)) {
        void loadProfile(profileId).catch((e: unknown) => setNotice(e instanceof Error ? e.message : "Could not refresh Dock workspace data."));
      }
    }).then((stop) => { if (disposed) stop(); else unlisten = stop; });
    return () => { disposed = true; unlisten(); };
  }, [nativeWindowLabel, activeProfileId, profiles]);
  useEffect(() => {
    if (nativeWindowLabel !== "main") return;
    let disposed = false;
    let unlisten: () => void = () => {};
    void api.onWorkspaceStateChanged(({ profileId }) => {
      if (profileId === activeProfileId || profiles.some((profile) => profile.id === profileId)) {
        void loadProfile(profileId).catch((e: unknown) => setNotice(e instanceof Error ? e.message : "Could not refresh workspace data."));
      }
    }).then((stop) => { if (disposed) stop(); else unlisten = stop; });
    return () => { disposed = true; unlisten(); };
  }, [nativeWindowLabel, activeProfileId, profiles]);
  useEffect(() => {
    const media = window.matchMedia?.("(prefers-color-scheme: dark)");
    const apply = () => {
      const root = document.documentElement;
      root.dataset.theme = resolveAppearance(themePreference.mode, media?.matches ?? false);
      root.dataset.density = themePreference.density;
      root.style.setProperty("--accent", themePreference.accent);
      root.style.setProperty("--accent-soft", `color-mix(in srgb, ${themePreference.accent} 12%, transparent)`);
    };
    apply(); media?.addEventListener("change", apply);
    return () => media?.removeEventListener("change", apply);
  }, [themePreference]);

  const filtered = useMemo(() => resources.filter((resource) => {
    const haystack = [resource.displayName, resource.description, resource.category, ...(resource.tags || []), ...resource.targets.map((t) => t.canonicalUri)].join(" ").toLowerCase();
    const matchQuery = query.trim().length === 0 || haystack.includes(query.trim().toLowerCase());
    const matchCategory = category === "All resources" || (category === "Favorites" ? resource.favorite : resource.category === category);
    const selectedCollectionId = selectedSmartCollections[activeProfileId];
    const selectedCollection = smartCollections.find((collection) => collection.id === selectedCollectionId);
    return matchQuery && matchCategory && (!selectedCollection || matchesSmartCollection(resource, selectedCollection.filters));
  }), [resources, query, category, smartCollections, activeProfileId, selectedSmartCollections]);
  const activeSmartCollection = smartCollections.find((collection) => collection.id === selectedSmartCollections[activeProfileId]);
  const selected = resources.find((resource) => resource.id === selectedId) ?? null;

  const update = async (id: string, patch: ResourcePatch) => {
    try { const changed = await api.update(id, { ...patch, profileId: activeProfileId }); setResources((current) => current.map((r) => r.id === id ? changed : r)); return true; }
    catch (e) { setNotice(e instanceof Error ? e.message : "Could not update resource."); return false; }
  };
  const duplicateResource = async (resource: Resource) => {
    const duplicate = await api.duplicateResource({ profileId: activeProfileId, resourceId: resource.id });
    const next = [...resources, duplicate];
    setResources(next); setSelectedId(duplicate.id);
    await persistLayout(next, cardWidths);
    return duplicate;
  };
  const previewRelocation = (request: ResourceRelocationPreviewRequest) => api.previewResourceRelocation(request);
  const commitRelocation = async (preview: ResourceRelocationPreview) => {
    const result = await api.commitResourceRelocation(preview);
    if (result.updatedTargetCount > 0) await loadProfile(activeProfileId);
    return result;
  };
  const toggleDockPin = async (resource: Resource) => {
    if (!activeProfileId) return;
    const cards = dockLayout?.profileId === activeProfileId ? [...dockLayout.cards] : [];
    const existing = cards.findIndex((card) => card.resourceId === resource.id);
    if (existing >= 0) cards.splice(existing, 1);
    else cards.push({ resourceId: resource.id, x: 0, y: cards.length, w: 1, h: 1, type: "icon" });
    const layout: DashboardLayout = { profileId: activeProfileId, surface: "dock", version: 1, cards: cards.map((card, index) => ({ ...card, x: 0, y: index })) };
    try { const saved = await api.saveLayout(layout); setDockLayout(saved); }
    catch (e) { setNotice(e instanceof Error ? e.message : "Could not update Dock pins."); }
  };
  const launch = async (resource: Resource, source: "dashboard" | "quick_launch" = "dashboard", targetId?: string): Promise<string | null> => {
    try {
      const result = targetId ? await api.launch(resource.id, source, activeProfileId, targetId) : await api.launch(resource.id, source, activeProfileId);
      if (result.status === "failed") {
        const code = result.errorCode || "launch_failed";
        const guidance = code === "target_not_found" || code === "file_not_found"
          ? "The saved target could not be found. Update its location in Explorer and try again."
          : code === "access_denied"
            ? "Windows denied access. Check the file or application permissions and try again."
            : `Windows could not open this resource (${code}). Verify its saved location and permissions, then try again.`;
        throw new Error(guidance);
      }
      if (source === "quick_launch" && result.status !== "handed_off") return "Windows has not confirmed the handoff yet. Check that the target is available, then try again.";
      setResources((current) => current.map((r) => r.id === resource.id ? { ...r, launchCount: (r.launchCount || 0) + 1, lastLaunchedUtc: result.launchedAtUtc } : r));
      setNotice(result.status === "handed_off" ? `Windows accepted the launch request for ${resource.displayName}.` : "Launch request sent.");
      return null;
    } catch (e) { const message = e instanceof Error ? e.message : "Launch failed."; setNotice(message); return message; }
  };
  const launchShortcut = async (shortcut: DynamicShortcut): Promise<string | null> => {
    const resource = resources.find((item) => item.id === shortcut.resourceId);
    if (!resource) return "This saved shortcut points to a resource that is no longer available in the active profile.";
    if (shortcut.targetId && !resource.targets.some((target) => target.id === shortcut.targetId && target.active)) return "This saved shortcut target is no longer active. Edit the shortcut and choose an available target.";
    return launch(resource, "quick_launch", shortcut.targetId);
  };
  const saveShortcut = async (event: FormEvent) => {
    event.preventDefault();
    if (!activeProfileId || !shortcutDraft.trim()) return;
    setShortcutSaving(true);
    try {
      const status = await api.setQuickLaunchHotkey(activeProfileId, shortcutDraft.trim());
      setHotkeyStatus(status);
      setShortcutDrafts((current) => ({ ...current, [activeProfileId]: { value: status.shortcut, dirty: false } }));
      if (!status.registered) setNotice(`${status.shortcut} could not be registered. ${status.activeShortcut ? `${status.activeShortcut} remains active.` : "The previous shortcut remains active."}`);
      else setNotice(`Quick Launch shortcut set to ${status.shortcut}.`);
    } catch (e) { setNotice(e instanceof Error ? e.message : "Could not update the Quick Launch shortcut."); }
    finally { setShortcutSaving(false); }
  };
  const saveTheme = async (theme: typeof DEFAULT_THEME) => {
    if (!activeProfileId) throw new Error("Select a profile before changing its appearance.");
    const changed = await api.setProfileTheme(activeProfileId, theme);
    setProfiles((current) => current.map((profile) => profile.id === changed.id ? changed : profile));
    setThemePreference(themeForProfile(changed));
    setNotice(`Appearance saved for ${changed.name}.`);
  };
  const remove = async (resource: Resource) => {
    if (!window.confirm(`Remove “${resource.displayName}” from this workspace?`)) return;
    try {
      await api.remove(resource.id, activeProfileId);
      const next = resources.filter((r) => r.id !== resource.id); setResources(next); setSelectedId(null);
      const nextTemplates = widgetTemplates.flatMap((template) => {
        if (template.kind === "related_resources" && template.config.resourceId === resource.id) return [];
        if (template.kind !== "resource_shortcuts" || !template.config.resourceIds.includes(resource.id)) return [template];
        const resourceIds = template.config.resourceIds.filter((id) => id !== resource.id);
        return resourceIds.length ? [{ ...template, config: { resourceIds } }] : [];
      });
      const retainedTemplateIds = new Set(nextTemplates.map((template) => template.id));
      const nextWidgets = dashboardWidgets.filter((widget) => retainedTemplateIds.has(widget.templateId));
      setWidgetTemplates(nextTemplates); setDashboardWidgets(nextWidgets);
      await persistLayout(next, cardWidths, cardHeights, nextTemplates, nextWidgets);
      setResourceRelationships((current) => current.filter((edge) => edge.sourceResourceId !== resource.id && edge.targetResourceId !== resource.id));
      if (dockLayout?.profileId === activeProfileId && dockLayout.cards.some((card) => card.resourceId === resource.id)) {
        const nextDock: DashboardLayout = { ...dockLayout, cards: dockLayout.cards.filter((card) => card.resourceId !== resource.id) };
        setDockLayout(await api.saveLayout(nextDock));
      }
    }
    catch (e) { setNotice(e instanceof Error ? e.message : "Could not remove resource."); }
  };
  const saveRelationship = async (relationship: ResourceRelationship) => {
    const saved = await api.saveResourceRelationship(relationship);
    setResourceRelationships((current) => current.some((item) => item.id === saved.id)
      ? current.map((item) => item.id === saved.id ? saved : item)
      : [...current, saved]);
    return saved;
  };
  const deleteRelationship = async (id: string) => {
    await api.deleteResourceRelationship(activeProfileId, id);
    setResourceRelationships((current) => current.filter((item) => item.id !== id));
  };
  const createSnapshot = async (name: string) => {
    const snapshot = await api.createWorkspaceSnapshot(activeProfileId, name);
    setWorkspaceSnapshots((current) => current.some((item) => item.id === snapshot.id) ? current : [...current, snapshot]);
    return snapshot;
  };
  const deleteSnapshot = async (id: string) => {
    await api.deleteWorkspaceSnapshot(activeProfileId, id);
    setWorkspaceSnapshots((current) => current.filter((item) => item.id !== id));
  };
  const restoreSnapshot = async (id: string) => {
    const result = await api.restoreWorkspaceSnapshot(activeProfileId, id);
    await loadProfile(activeProfileId);
    setNotice(result.missingResourceIds.length
      ? `Workspace layouts restored. ${result.missingResourceIds.length} saved resource reference(s) are no longer in this profile.`
      : "Workspace layouts restored.");
    return result;
  };
  const reorder = (overId: string) => {
    if (!dragId || dragId === overId) return;
    const next = [...resources]; const from = next.findIndex((r) => r.id === dragId); const to = next.findIndex((r) => r.id === overId);
    const [moved] = next.splice(from, 1); next.splice(to, 0, moved); setResources(next); setDragId(null);
    void persistLayout(next, cardWidths);
  };
  const moveCard = (id: string, offset: number) => {
    const from = resources.findIndex((resource) => resource.id === id); const to = Math.max(0, Math.min(resources.length - 1, from + offset));
    if (from < 0 || from === to) return;
    const next = [...resources]; const [item] = next.splice(from, 1); next.splice(to, 0, item); setResources(next); void persistLayout(next, cardWidths);
  };
  const resizeCard = (id: string, width: number) => {
    const next = { ...cardWidths, [id]: width }; setCardWidths(next); void persistLayout(resources, next);
  };
  const resizeCardHeight = (id: string, height: number) => {
    const next = { ...cardHeights, [id]: height }; setCardHeights(next); void persistLayout(resources, cardWidths, next);
  };
  const openDock = async () => {
    try { await api.setDockVisibility(true); }
    catch (e) { setNotice(e instanceof Error ? e.message : "Could not open the workspace Dock."); }
  };
  const switchProfile = async (profileId: string) => {
    if (profileId === activeProfileId) return;
    setLoading(true); setNotice("");
    try { await loadProfile(profileId, true); setQuery(""); }
    catch (e) { setNotice(e instanceof Error ? e.message : "Could not switch profile."); }
    finally { setLoading(false); }
  };
  const persistLayout = async (items: Resource[], widths: Record<string, number>, heights: Record<string, number> = cardHeights, templates: WidgetTemplate[] = widgetTemplates, widgets: DashboardWidget[] = dashboardWidgets) => {
    if (!activeProfileId) return;
    try { await api.saveLayout(makeLayout(activeProfileId, items, widths, heights, templates, widgets)); }
    catch (e) { setNotice(e instanceof Error ? e.message : "Could not save dashboard layout."); }
  };
  const saveDashboardWidgets = async (templates: WidgetTemplate[], widgets: DashboardWidget[]) => {
    if (!activeProfileId) return;
    try {
      const saved = await api.saveLayout(makeLayout(activeProfileId, resources, cardWidths, cardHeights, templates, widgets));
      setWidgetTemplates(saved.widgetTemplates || templates); setDashboardWidgets(saved.widgets || widgets);
    } catch (e) { setNotice(e instanceof Error ? e.message : "Could not save dashboard widgets."); }
  };

  const closeQuickLaunch = () => {
    setQuickLaunchOpen(false);
    if (nativeWindowLabel === "quick-launch") void api.hideCurrentWindow();
  };

  if (nativeWindowLabel === "dock") return <DockWindow profileId={activeProfileId} profileName={profiles.find((profile) => profile.id === activeProfileId)?.name || "Workspace"} resources={resources} layout={dockLayout} hotkeyStatus={hotkeyStatus} onLaunch={(resource) => api.launch(resource.id, "dock", activeProfileId)} onClose={() => void api.hideCurrentWindow()} onRefresh={() => void loadProfile().catch((e: unknown) => setNotice(e instanceof Error ? e.message : "Could not refresh Dock data."))} />;

  if (nativeWindowLabel === "quick-launch") return quickLaunchOpen
    ? <QuickLaunchOverlay key={quickLaunchSignal} presentation="window" resources={resources} shortcuts={dynamicShortcuts} profileId={activeProfileId} hotkeyStatus={hotkeyStatus} onClose={closeQuickLaunch} onLaunch={(resource) => launch(resource, "quick_launch")} onLaunchShortcut={launchShortcut} />
    : <div className="quick-launch-window" />;

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><span className="brand-symbol"><span /><span /><span /><span /></span><span className="brand-name">Workspace</span></div>
      <div className="profile-select"><span className="profile-avatar">{profiles.find((profile) => profile.id === activeProfileId)?.name?.slice(0, 1) || "P"}</span><span className="profile-label"><b>{profiles.find((profile) => profile.id === activeProfileId)?.name || "Personal"}</b><small>Workspace profile</small></span><Icon name="chevron" size={15} /><select className="profile-switch" aria-label="Switch profile" value={activeProfileId} onChange={(event) => void switchProfile(event.target.value)}>{profiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.name}</option>)}</select></div>
      <div className="nav-label">WORKSPACE</div>
      <nav aria-label="Primary navigation">{nav.map((item) => <button key={item.label} className={`nav-item ${section === item.label ? "active" : ""}`} onClick={() => item.label === "Quick Launch" ? setQuickLaunchOpen(true) : setSection(item.label)}><Icon name={item.icon} size={18} />{item.label}{item.label === "Quick Launch" && <kbd>⌥ Space</kbd>}</button>)}</nav>
      <div className="nav-label collections-label">COLLECTIONS <button className="tiny-plus" onClick={() => setSection("Smart Collections")} aria-label="Add smart collection"><Icon name="plus" size={14} /></button></div>
      {["All resources", "Favorites", "Work", "Web", "Apps"].map((item, index) => <button key={item} className={`collection ${category === item && !selectedSmartCollections[activeProfileId] ? "selected" : ""}`} onClick={() => { setCategory(item); setSelectedSmartCollections((current) => ({ ...current, [activeProfileId]: null })); setSection("Dashboard"); }}><span className={`collection-dot d${index}`} />{item}<span className="collection-count">{item === "Favorites" ? resources.filter((r) => r.favorite).length : item === "All resources" ? resources.length : resources.filter((r) => r.category === item).length}</span></button>)}
      {smartCollections.map((collection, index) => <button key={collection.id} aria-label={collection.name} className={`collection smart-collection-nav ${selectedSmartCollections[activeProfileId] === collection.id ? "selected" : ""}`} onClick={() => { setCategory("All resources"); setSelectedSmartCollections((current) => ({ ...current, [activeProfileId]: collection.id })); setSection("Dashboard"); }}><span className={`collection-dot d${index % 5}`} />{collection.name}<span className="collection-count">{resources.filter((resource) => matchesSmartCollection(resource, collection.filters)).length}</span></button>)}
      <div className="sidebar-bottom"><button className="nav-item" onClick={() => setSettingsOpen(true)}><Icon name="moon" size={18} />Appearance</button><button className="nav-item" aria-expanded={settingsOpen} onClick={() => setSettingsOpen((open) => !open)}><Icon name="settings" size={18} />Settings</button>{settingsOpen && <><ProfileThemeSettings value={themePreference} onSave={saveTheme} /><form className="shortcut-settings" onSubmit={(event) => void saveShortcut(event)}><label htmlFor="quick-launch-shortcut">Quick Launch shortcut · {selectedProfile?.name || "Profile"}</label><input id="quick-launch-shortcut" value={shortcutDraft} onChange={(event) => setShortcutDrafts((current) => ({ ...current, [activeProfileId]: { value: event.target.value, dirty: true } }))} placeholder="Alt+Space" /><small>Use modifiers such as Ctrl, Alt, or Shift.</small><button className="button primary" disabled={shortcutSaving || !activeProfileId}>{shortcutSaving ? "Saving…" : "Save shortcut"}</button></form><ProfileArchiveControls profiles={profiles} activeProfileId={activeProfileId} onImported={(id) => { void loadProfile(id); }} /></>}<div className="sidebar-user"><div className="user-avatar">A</div><div><b>Alex Morgan</b><small>Local workspace</small></div><button className="icon-button" aria-label="More account options"><Icon name="more" /></button></div></div>
    </aside>

    <main className="main-area">
      <header className="topbar"><div className="breadcrumbs"><span>Workspace</span><Icon name="chevron" size={13} /><b>{section}</b></div><div className="top-actions"><button className="button quiet" onClick={() => setSettingsOpen(true)} aria-label="Open appearance settings" title="Appearance settings"><Icon name="moon" size={16} /></button><button className="help-button" aria-label="Help">?</button><button className="profile-chip"><span className="user-avatar mini">A</span><span>Alex</span><Icon name="chevron" size={13} /></button></div></header>
      <div className="content-wrap">
        {hotkeyStatus && !hotkeyStatus.registered && <div className="hotkey-registration-warning" role="status">{hotkeyStatus.shortcut} could not be registered. {hotkeyStatus.activeShortcut ? `${hotkeyStatus.activeShortcut} remains active.` : "Use the Quick Launch button in the sidebar instead."}{hotkeyStatus.errorCode ? ` (${hotkeyStatus.errorCode})` : ""}</div>}
{section === "Device paths" ? <DevicePathSettings /> : section === "Extensions" ? <ExtensionsRegistry /> : section === "Analytics" ? <AnalyticsView profileId={activeProfileId} /> : section === "Import resources" ? <ResourceImportAssistant profileId={activeProfileId} onImported={(message) => { void loadProfile(activeProfileId); setNotice(message); }} /> : section === "Explorer" ? <ResourceExplorer resources={resources} profileId={activeProfileId} onSelect={(resource) => setSelectedId(resource.id)} onAdd={() => setShowAdd(true)} onUpdate={update} onDuplicate={duplicateResource} onRelocationPreview={previewRelocation} onRelocationCommit={commitRelocation} /> : section === "Smart Collections" ? <SmartCollectionsManager profileId={activeProfileId} collections={smartCollections} resources={resources} onClose={() => setSection("Dashboard")} onSave={async (collection) => { const saved = await api.saveSmartCollection(collection); setSmartCollections((current) => current.some((item) => item.id === saved.id) ? current.map((item) => item.id === saved.id ? saved : item) : [...current, saved]); setCategory("All resources"); setSelectedSmartCollections((current) => ({ ...current, [activeProfileId]: saved.id })); return saved; }} onDelete={async (id) => { await api.deleteSmartCollection(activeProfileId, id); setSmartCollections((current) => current.filter((item) => item.id !== id)); setSelectedSmartCollections((current) => current[activeProfileId] === id ? { ...current, [activeProfileId]: null } : current); }} /> : section === "Quick Launch shortcuts" ? <DynamicShortcutsManager profileId={activeProfileId} resources={resources} shortcuts={dynamicShortcuts} onClose={() => setSection("Dashboard")} onSave={async (shortcut) => { const saved = await api.saveDynamicShortcut(shortcut); setDynamicShortcuts((current) => current.some((item) => item.id === saved.id) ? current.map((item) => item.id === saved.id ? saved : item) : [...current, saved]); return saved; }} onDelete={async (id) => { await api.deleteDynamicShortcut(activeProfileId, id); setDynamicShortcuts((current) => current.filter((item) => item.id !== id)); }} /> : section === "Workspace sequences" ? <WorkspaceSequencesManager profileId={activeProfileId} sequences={workspaceSequences} resources={resources} onClose={() => setSection("Dashboard")} onSave={async (sequence) => { const saved = await api.saveWorkspaceSequence(sequence); setWorkspaceSequences((current) => current.some((item) => item.id === saved.id) ? current.map((item) => item.id === saved.id ? saved : item) : [...current, saved]); return saved; }} onDelete={async (id) => { await api.deleteWorkspaceSequence(activeProfileId, id); setWorkspaceSequences((current) => current.filter((item) => item.id !== id)); }} onRun={(id) => api.runWorkspaceSequence(activeProfileId, id)} /> : section === "Resource graph & snapshots" ? <WorkspaceMapManager profileId={activeProfileId} resources={resources} relationships={resourceRelationships} snapshots={workspaceSnapshots} onClose={() => setSection("Dashboard")} onSelectResource={(id) => { setSelectedId(id); setSection("Explorer"); }} onSaveRelationship={saveRelationship} onDeleteRelationship={deleteRelationship} onCreateSnapshot={createSnapshot} onDeleteSnapshot={deleteSnapshot} onRestoreSnapshot={restoreSnapshot} /> : section === "Dock" ? <section className="dock-manager"><div className="welcome-row"><div><h1>Workspace Dock</h1><p>Choose the resources pinned in the independent floating Dock.</p></div><button className="button primary" onClick={() => void openDock()}>Open Dock</button></div><div className="dock-manager-list">{resources.map((resource) => { const pinned = dockLayout?.cards.some((card) => card.resourceId === resource.id) || false; return <label className="dock-manager-row" key={resource.id}><input type="checkbox" checked={pinned} onChange={() => void toggleDockPin(resource)} /><span><b>{resource.displayName}</b><small>{resource.targets[0]?.canonicalUri || resource.description}</small></span><em>{resource.resourceType}</em></label>; })}{resources.length === 0 && <p className="empty-state">Add resources before pinning them to the Dock.</p>}</div></section> : <>
          <div className="welcome-row"><div><h1>{section === "Quick Launch" ? "Quick Launch" : greeting()}</h1><p>{section === "Quick Launch" ? "Find a resource and open it in its default app." : "Everything you need, right where you left it."}</p></div><button className="button primary add-button" onClick={() => setShowAdd(true)}><Icon name="plus" size={17} /> Add resource</button></div>
          <div className={`search-box ${section === "Quick Launch" ? "command-search" : ""}`}><Icon name="search" size={19} /><input id="global-search" placeholder={section === "Quick Launch" ? "Search resources, aliases, tags…" : "Search your workspace"} value={query} onChange={(e) => setQuery(e.target.value)} /><span className="search-shortcut">{section === "Quick Launch" ? "ESC to close" : "⌘ K"}</span></div>
          <div className="section-heading"><div><h2>{query ? "Search results" : activeSmartCollection?.name || (category === "Favorites" ? "Favorites" : "Pinned resources")}</h2><span>{filtered.length} {filtered.length === 1 ? "resource" : "resources"}{activeSmartCollection ? " · live saved query" : ""}</span></div><button className="text-button" onClick={() => { setSection("Explorer"); setCategory("All resources"); setSelectedSmartCollections((current) => ({ ...current, [activeProfileId]: null })); }}>View all <Icon name="arrow" size={15} /></button></div>
          {notice && <div className="notice" role="status"><span>{notice}</span><button className="icon-button" onClick={() => setNotice("")} aria-label="Dismiss"><Icon name="close" size={15} /></button></div>}
          {section === "Dashboard" && <DashboardWidgets profileId={activeProfileId} resources={resources} relationships={resourceRelationships} templates={widgetTemplates} widgets={dashboardWidgets} onChange={(templates, widgets) => void saveDashboardWidgets(templates, widgets)} onLaunch={(resource) => { void launch(resource); }} onInspect={(id) => setSelectedId(id)} onNotice={setNotice} />}
          {loading ? <div className="loading-state">Loading your workspace…</div> : filtered.length === 0 ? <div className="empty-state"><span className="empty-icon"><Icon name="folder" size={24} /></span><h3>{query ? "No matching resources" : "Your workspace is ready"}</h3><p>{query ? "Try another name, type, tag or location." : "Add a file, folder, app or secure web link to get started."}</p>{!query && <button className="button primary" onClick={() => setShowAdd(true)}><Icon name="plus" size={16} /> Add your first resource</button>}</div> : <div className="resource-grid" aria-label="Resources">
            {filtered.map((resource) => <article key={resource.id} style={{ gridColumn: `span ${Math.min(cardWidths[resource.id] || 4, gridColumns)}`, gridRow: `span ${Math.max(1, Math.min(3, cardHeights[resource.id] || 1))}` }} className={`resource-card ${selectedId === resource.id ? "selected" : ""}`} draggable onDragStart={() => setDragId(resource.id)} onDragOver={(e) => e.preventDefault()} onDrop={() => reorder(resource.id)} onClick={() => setSelectedId(resource.id)} onDoubleClick={() => void launch(resource)}>
              <div className="card-top"><ResourceMark resource={resource} /><div className="card-controls"><button className="move-card" aria-label={`Move ${resource.displayName} earlier`} title="Move earlier" disabled={resources[0]?.id === resource.id} onClick={(e) => { e.stopPropagation(); moveCard(resource.id, -1); }}>↑</button><button className="move-card" aria-label={`Move ${resource.displayName} later`} title="Move later" disabled={resources[resources.length - 1]?.id === resource.id} onClick={(e) => { e.stopPropagation(); moveCard(resource.id, 1); }}>↓</button><button className={`favorite-button ${resource.favorite ? "is-favorite" : ""}`} title={resource.favorite ? "Remove from favorites" : "Add to favorites"} aria-label={resource.favorite ? `Remove ${resource.displayName} from favorites` : `Add ${resource.displayName} to favorites`} onClick={(e) => { e.stopPropagation(); void update(resource.id, { favorite: !resource.favorite }); }}><Icon name="star" size={16} /></button></div></div>
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
      {selected ? <><div className="inspector-resource"><ResourceMark resource={selected} /><div><h3>{selected.displayName}</h3><span>{selected.resourceType}</span></div></div><button className="button primary inspector-launch" onClick={() => void launch(selected)}><Icon name="arrow" size={16} /> Open resource</button><div className="inspector-section"><div className="inspector-label">LOCATION</div>{editingResource === selected.id && <ResourceEditorForm key={selected.id} resource={selected} onCancel={() => setEditingResource(null)} onSave={async (patch) => { const changed = await api.update(selected.id, { ...patch, profileId: activeProfileId }); setResources((current) => current.map((item) => item.id === selected.id ? changed : item)); }} />}{selected.targets.length ? selected.targets.map((target) => <div className="inspector-target" key={target.id}><span className="target-type">{target.kind}{target.active ? " · active" : " · inactive"}</span><div className="location-value">{target.canonicalUri}</div><button className="subtle-action" onClick={() => void navigator.clipboard?.writeText(target.canonicalUri)}>Copy target path</button></div>) : <div className="location-value">No location available</div>}</div><div className="inspector-section"><div className="inspector-label">CARD WIDTH</div><div className="size-options" role="group" aria-label="Dashboard card width">{[{ value: 3, label: "Compact" }, { value: 4, label: "Standard" }, { value: 6, label: "Wide" }].map((item) => <button key={item.value} className={(cardWidths[selected.id] || 4) === item.value ? "chosen" : ""} onClick={() => resizeCard(selected.id, item.value)}>{item.label}</button>)}</div><div className="inspector-label card-height-label">CARD HEIGHT</div><div className="size-options" role="group" aria-label="Dashboard card height">{[{ value: 1, label: "Short" }, { value: 2, label: "Tall" }, { value: 3, label: "Extra tall" }].map((item) => <button key={item.value} className={(cardHeights[selected.id] || 1) === item.value ? "chosen" : ""} onClick={() => resizeCardHeight(selected.id, item.value)}>{item.label}</button>)}</div></div><div className="inspector-section"><div className="inspector-label">DETAILS</div><InfoRow label="Collection" value={selected.category || "Uncategorized"} /><InfoRow label="Launches" value={String(selected.launchCount || 0)} /><InfoRow label="Last launched" value={selected.lastLaunchedUtc ? relativeTime(selected.lastLaunchedUtc) : "No launch history"} /></div><div className="inspector-section"><ResourceMetadataPanel resource={selected} /></div><div className="inspector-footer"><button className="subtle-action" aria-expanded={editingResource === selected.id} onClick={() => setEditingResource((current) => current === selected.id ? null : selected.id)}>{editingResource === selected.id ? "Close editor" : "Edit details"}</button><button className="subtle-action danger" onClick={() => void remove(selected)}>Remove</button></div></> : <div className="inspector-empty"><span className="empty-icon"><Icon name="grid" /></span><p>Select a resource to inspect its details.</p></div>}
    </aside>
    {showAdd && <AddResourceDialog profileId={activeProfileId} onClose={() => setShowAdd(false)} onCreate={(resource) => { const next = [...resources, resource]; setResources(next); setSelectedId(resource.id); void persistLayout(next, cardWidths); setNotice(`${resource.displayName} added to your workspace.`); }} />}
    {quickLaunchOpen && <QuickLaunchOverlay key={quickLaunchSignal} resources={resources} shortcuts={dynamicShortcuts} profileId={activeProfileId} hotkeyStatus={hotkeyStatus} onClose={closeQuickLaunch} onLaunch={(resource) => launch(resource, "quick_launch")} onLaunchShortcut={launchShortcut} />}
  </div>;
}

function greeting() { const hour = new Date().getHours(); return hour < 12 ? "Good morning, Alex" : hour < 18 ? "Good afternoon, Alex" : "Good evening, Alex"; }
function relativeTime(value: string) { const elapsed = Math.max(0, Date.now() - new Date(value).getTime()); const minutes = Math.floor(elapsed / 60000); if (minutes < 1) return "Just now"; if (minutes < 60) return `${minutes} min ago`; const hours = Math.floor(minutes / 60); return hours < 24 ? `${hours} hr ago` : `${Math.floor(hours / 24)} days ago`; }
function restoreLayout(resources: Resource[], layout: DashboardLayout | null) {
  if (!layout || layout.surface !== "dashboard") return { resources, widths: {} as Record<string, number>, heights: {} as Record<string, number> };
  const cards = [...layout.cards].sort((a, b) => a.y - b.y || a.x - b.x);
  const rank = new Map(cards.map((card, index) => [card.resourceId, index]));
  const ordered = [...resources].sort((a, b) => (rank.get(a.id) ?? Number.MAX_SAFE_INTEGER) - (rank.get(b.id) ?? Number.MAX_SAFE_INTEGER));
  const widths = Object.fromEntries(cards.map((card) => [card.resourceId, Math.max(3, Math.min(8, card.w))]));
  const heights = Object.fromEntries(cards.map((card) => [card.resourceId, Math.max(1, Math.min(3, card.h))]));
  return { resources: ordered, widths, heights, widgetTemplates: layout.widgetTemplates || [], widgets: layout.widgets || [] };
}
function makeLayout(profileId: string, resources: Resource[], widths: Record<string, number>, heights: Record<string, number> = {}, widgetTemplates: WidgetTemplate[] = [], widgets: DashboardWidget[] = []): DashboardLayout {
  let x = 0; let y = 0;
  const cards = resources.map((resource) => {
    const w = Math.max(3, Math.min(8, widths[resource.id] || 4));
    if (x + w > 12) { x = 0; y += 1; }
    const h = Math.max(1, Math.min(3, heights[resource.id] || 1));
    const card = { resourceId: resource.id, x, y, w, h, type: (w < 4 ? "compact" : w > 4 ? "info" : "icon") as "compact" | "info" | "icon" };
    x += w;
    if (x >= 12) { x = 0; y += 1; }
    return card;
  });
  return { profileId, surface: "dashboard", version: 1, cards, widgetTemplates, widgets };
}
function InfoRow({ label, value }: { label: string; value: string }) { return <div className="info-row"><span>{label}</span><b>{value}</b></div>; }
export function columnsForViewport(width: number) { return width <= 760 ? 4 : width <= 1180 ? 6 : 12; }
