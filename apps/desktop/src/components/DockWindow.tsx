import { useEffect, useMemo, useRef, useState } from "react";
import type { DashboardLayout, LaunchResult, QuickLaunchHotkeyStatus } from "@sw/contracts";
import type { Monitor } from "@tauri-apps/api/window";
import type { Resource } from "../lib/contracts";
import { api } from "../lib/api";

type DockEdge = "left" | "right" | "top" | "bottom";
type DockPreferences = { edge: DockEdge; orientation: "vertical" | "horizontal"; monitor: number; iconSize: number; spacing: number; opacity: number; alwaysOnTop: boolean; autoHide: boolean };
const DEFAULTS: DockPreferences = { edge: "right", orientation: "vertical", monitor: 0, iconSize: 48, spacing: 9, opacity: 0.96, alwaysOnTop: true, autoHide: false };
const prefsKey = (profileId: string) => `swl-dock-preferences-${profileId}-v1`;
function readPreferences(profileId: string): DockPreferences {
  try { return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(prefsKey(profileId)) || "{}") }; } catch { return DEFAULTS; }
}
export function createDockAutoHideScheduler(hide: () => void, isFocused: () => Promise<boolean>, delay = 450) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  return {
    focusChanged(focused: boolean) {
      if (timer !== undefined) { clearTimeout(timer); timer = undefined; }
      if (focused) return;
      timer = setTimeout(() => {
        timer = undefined;
        void isFocused().then((stillFocused) => { if (!stillFocused) hide(); });
      }, delay);
    },
    dispose() { if (timer !== undefined) clearTimeout(timer); timer = undefined; },
  };
}

export function DockWindow({ profileId, profileName, resources, layout, hotkeyStatus, onLaunch, onClose, onRefresh }: {
  profileId: string; profileName: string; resources: Resource[]; layout: DashboardLayout | null;
  hotkeyStatus: QuickLaunchHotkeyStatus | null;
  onLaunch: (resource: Resource) => Promise<LaunchResult>; onClose: () => void; onRefresh: () => void;
}) {
  const [preferencesState, setPreferencesState] = useState(() => ({ profileId, value: readPreferences(profileId) }));
  const preferences = preferencesState.profileId === profileId ? preferencesState.value : readPreferences(profileId);
  const [monitors, setMonitors] = useState<Monitor[]>([]);
  const [error, setError] = useState("");
  const refreshRef = useRef(onRefresh);
  refreshRef.current = onRefresh;
  const pinned = useMemo(() => {
    const byId = new Map(resources.map((resource) => [resource.id, resource]));
    return (layout?.cards || []).slice().sort((a, b) => a.y - b.y || a.x - b.x).map((card) => byId.get(card.resourceId)).filter((resource): resource is Resource => Boolean(resource));
  }, [layout, resources]);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let disposed = false;
    void import("@tauri-apps/api/window").then(async ({ availableMonitors }) => {
      const screens = await availableMonitors();
      if (!disposed) setMonitors(screens);
    });
    return () => { disposed = true; };
  }, []);
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void import("@tauri-apps/api/window").then(async ({ getCurrentWindow }) => {
      const stop = await getCurrentWindow().onFocusChanged(({ payload: focused }) => { if (focused) refreshRef.current(); });
      if (disposed) stop(); else unlisten = stop;
    });
    return () => { disposed = true; unlisten?.(); };
  }, []);
  useEffect(() => {
    localStorage.setItem(prefsKey(profileId), JSON.stringify(preferences));
    if (!("__TAURI_INTERNALS__" in window)) return;
    let disposed = false;
    void import("@tauri-apps/api/window").then(async ({ availableMonitors, getCurrentWindow, LogicalPosition, LogicalSize }) => {
      const win = getCurrentWindow();
      const screens = await availableMonitors();
      const monitor = screens[preferences.monitor] || screens[0];
      if (!monitor || disposed) return;
      const area = monitor.workArea;
      const scale = monitor.scaleFactor;
      const origin = area.position.toLogical(scale);
      const extent = area.size.toLogical(scale);
      const cross = preferences.iconSize + 24;
      const along = Math.max(preferences.iconSize + 24, Math.min(pinned.length, 9) * (preferences.iconSize + preferences.spacing) + 24);
      const width = preferences.orientation === "vertical" ? cross : along;
      const height = preferences.orientation === "vertical" ? along : cross;
      const x = preferences.edge === "left" ? origin.x : preferences.edge === "right" ? origin.x + extent.width - width : origin.x + (extent.width - width) / 2;
      const y = preferences.edge === "top" ? origin.y : preferences.edge === "bottom" ? origin.y + extent.height - height : origin.y + (extent.height - height) / 2;
      await win.setSize(new LogicalSize(width, height));
      await win.setPosition(new LogicalPosition(x, y));
      await win.setAlwaysOnTop(preferences.alwaysOnTop);
    }).catch(() => setError("Could not position the Dock on the selected display."));
    return () => { disposed = true; };
  }, [preferences, pinned.length, profileId]);
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window) || !preferences.autoHide) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let scheduler: ReturnType<typeof createDockAutoHideScheduler> | undefined;
    void import("@tauri-apps/api/window").then(async ({ getCurrentWindow }) => {
      const dock = getCurrentWindow();
      scheduler = createDockAutoHideScheduler(() => { if (!disposed) void api.hideCurrentWindow(); }, () => dock.isFocused());
      const stop = await dock.onFocusChanged(({ payload: focused }) => {
        scheduler?.focusChanged(focused);
      });
      if (disposed) stop(); else unlisten = stop;
    });
    return () => { disposed = true; scheduler?.dispose(); unlisten?.(); };
  }, [preferences.autoHide]);
  const update = <K extends keyof DockPreferences>(key: K, value: DockPreferences[K]) => setPreferencesState({ profileId, value: { ...preferences, [key]: value } });
  const open = async (resource: Resource) => {
    setError("");
    try {
      const result = await onLaunch(resource);
      if (result.status === "failed") setError(`Could not open ${resource.displayName}. Check its saved target and try again (${result.errorCode || "launch_failed"}).`);
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not open this resource. Check its saved target and try again."); }
  };

  return <main className="dock-window" style={{ opacity: preferences.opacity }} aria-label="Workspace Dock">
    <header className="dock-header"><span className="dock-grip" aria-hidden="true">⠿</span><span className="dock-profile" title={hotkeyStatus ? `Quick Launch shortcut: ${hotkeyStatus.shortcut}` : profileName}>{profileName}</span><button className="dock-close" aria-label="Hide Dock" onClick={onClose}>×</button></header>
    {hotkeyStatus && !hotkeyStatus.registered && <div className="dock-error" role="status">Quick Launch shortcut unavailable: {hotkeyStatus.activeShortcut || hotkeyStatus.shortcut}</div>}
    {error && <div className="dock-error" role="alert">{error}</div>}
    <div className={`dock-items ${preferences.orientation}`}>
      {pinned.map((resource) => <button key={resource.id} className="dock-item" title={resource.displayName} aria-label={`Open ${resource.displayName}`} style={{ width: preferences.iconSize, height: preferences.iconSize, margin: preferences.spacing / 2 }} onClick={() => void open(resource)}>
        <span>{resource.resourceType === "folder" ? "▱" : resource.resourceType === "url" ? "◎" : resource.displayName.slice(0, 1).toUpperCase()}</span>
      </button>)}
      {pinned.length === 0 && <span className="dock-empty">Pin items from Dashboard</span>}
    </div>
    <details className="dock-settings"><summary aria-label="Dock settings">⚙</summary><section>
      <label>Screen edge<select value={preferences.edge} onChange={(event) => update("edge", event.target.value as DockEdge)}><option value="left">Left</option><option value="right">Right</option><option value="top">Top</option><option value="bottom">Bottom</option></select></label>
      <label>Orientation<select value={preferences.orientation} onChange={(event) => update("orientation", event.target.value as DockPreferences["orientation"])}><option value="vertical">Vertical</option><option value="horizontal">Horizontal</option></select></label>
      <label>Monitor<select value={preferences.monitor} onChange={(event) => update("monitor", Number(event.target.value))}>{(monitors.length ? monitors.map((monitor) => monitor.name) : ["Primary display"]).map((name, index) => <option key={index} value={index}>{name || `Display ${index + 1}`}</option>)}</select></label>
      <label>Icon size<input type="range" min="32" max="72" value={preferences.iconSize} onChange={(event) => update("iconSize", Number(event.target.value))} /></label>
      <label>Spacing<input type="range" min="2" max="20" value={preferences.spacing} onChange={(event) => update("spacing", Number(event.target.value))} /></label>
      <label>Opacity<input type="range" min="0.55" max="1" step="0.05" value={preferences.opacity} onChange={(event) => update("opacity", Number(event.target.value))} /></label>
      <label className="dock-check"><input type="checkbox" checked={preferences.alwaysOnTop} onChange={(event) => update("alwaysOnTop", event.target.checked)} /> Always on top</label>
      <label className="dock-check"><input type="checkbox" checked={preferences.autoHide} onChange={(event) => update("autoHide", event.target.checked)} /> Hide when unfocused</label>
    </section></details>
  </main>;
}
