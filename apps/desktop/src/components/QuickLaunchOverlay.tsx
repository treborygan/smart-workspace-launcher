import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { Icon } from "./Icon";
import { api } from "../lib/api";
import type { Resource } from "../lib/contracts";
import type { DynamicShortcut, QuickLaunchHotkeyStatus } from "@sw/contracts";

type Props = {
  resources: Resource[];
  profileId?: string;
  hotkeyStatus: QuickLaunchHotkeyStatus | null;
  shortcuts?: DynamicShortcut[];
  presentation?: "overlay" | "window";
  onClose: () => void;
  onLaunch: (resource: Resource) => Promise<string | null>;
  onLaunchShortcut?: (shortcut: DynamicShortcut) => Promise<string | null>;
};

export function QuickLaunchOverlay({ resources, profileId, hotkeyStatus, shortcuts = [], presentation = "overlay", onClose, onLaunch, onLaunchShortcut }: Props) {
  const inputRef = useRef<HTMLInputElement>(null);
  const requestRef = useRef(0);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Resource[]>(resources);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [launchError, setLaunchError] = useState("");
  const [launching, setLaunching] = useState(false);
  const shortcutEntries = shortcuts.flatMap((shortcut) => {
    const resource = resources.find((item) => item.id === shortcut.resourceId);
    if (!resource || (shortcut.targetId && !resource.targets.some((target) => target.id === shortcut.targetId && target.active))) return [];
    const target = resource.targets.find((item) => item.id === shortcut.targetId);
    const queryText = query.trim().toLocaleLowerCase();
    const searchable = [shortcut.name, resource.displayName, resource.description, target?.canonicalUri].filter(Boolean).join(" ").toLocaleLowerCase();
    return !queryText || searchable.includes(queryText) ? [{ shortcut, resource, target }] : [];
  }).sort((a, b) => a.shortcut.name.localeCompare(b.shortcut.name, undefined, { sensitivity: "base" }));
  const entryCount = shortcutEntries.length + results.length;

  useEffect(() => { inputRef.current?.focus(); }, []);
  useEffect(() => {
    if (presentation !== "window" || !("__TAURI_INTERNALS__" in window)) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void import("@tauri-apps/api/window").then(async ({ getCurrentWindow }) => {
      const stopListening = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
        if (focused) requestAnimationFrame(() => inputRef.current?.focus());
      });
      if (disposed) stopListening(); else unlisten = stopListening;
    });
    return () => { disposed = true; unlisten?.(); };
  }, [presentation]);
  useEffect(() => {
    const requestId = ++requestRef.current;
    const timer = window.setTimeout(() => {
      setLoading(true); setError("");
      void api.search(query, profileId, resources).then((matches) => {
        if (requestId !== requestRef.current) return;
        setResults(matches); setSelectedIndex(0);
      }).catch((cause: unknown) => {
        if (requestId !== requestRef.current) return;
        setError(cause instanceof Error ? cause.message : "Search is unavailable."); setResults([]);
      }).finally(() => { if (requestId === requestRef.current) setLoading(false); });
    }, query ? 80 : 0);
    return () => window.clearTimeout(timer);
  }, [query, profileId, resources]);

  const launchSelected = async (resource: Resource) => {
    if (launching) return;
    setLaunching(true); setLaunchError("");
    try {
      const failure = await onLaunch(resource);
      if (failure) setLaunchError(failure); else onClose();
    } catch (cause) { setLaunchError(cause instanceof Error ? cause.message : "Could not hand off this resource. Check that its target still exists and try again."); }
    finally { setLaunching(false); }
  };
  const launchShortcut = async (shortcut: DynamicShortcut) => {
    if (launching || !onLaunchShortcut) return;
    setLaunching(true); setLaunchError("");
    try {
      const failure = await onLaunchShortcut(shortcut);
      if (failure) setLaunchError(failure); else onClose();
    } catch (cause) { setLaunchError(cause instanceof Error ? cause.message : "Could not hand off this saved resource. Check its location and try again."); }
    finally { setLaunching(false); }
  };
  const launchEntry = async (index: number) => {
    if (index < shortcutEntries.length) await launchShortcut(shortcutEntries[index].shortcut);
    else if (results[index - shortcutEntries.length]) await launchSelected(results[index - shortcutEntries.length]);
  };
  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") { event.preventDefault(); setSelectedIndex((index) => Math.min(index + 1, Math.max(0, entryCount - 1))); }
    if (event.key === "ArrowUp") { event.preventDefault(); setSelectedIndex((index) => Math.max(0, index - 1)); }
    if (event.key === "Enter" && entryCount > selectedIndex) { event.preventDefault(); void launchEntry(selectedIndex); }
    if (event.key === "Escape") { event.preventDefault(); onClose(); }
  };

  return <div className={presentation === "window" ? "quick-launch-window" : "quick-launch-backdrop"} role="presentation" onMouseDown={(event) => presentation === "overlay" && event.target === event.currentTarget && onClose()}>
    <section className={`quick-launch ${presentation === "window" ? "in-window" : ""}`} role="dialog" aria-modal="true" aria-label="Quick Launch">
      <div className="quick-launch-search"><Icon name="search" size={20} /><input ref={inputRef} aria-label="Search resources" placeholder="Search resources, aliases, tags…" value={query} onChange={(event) => setQuery(event.target.value)} onKeyDown={handleKeyDown} /><kbd>ESC</kbd></div>
      {hotkeyStatus && !hotkeyStatus.registered && <div className="quick-launch-warning" role="status">{hotkeyStatus.shortcut} could not be registered. {hotkeyStatus.activeShortcut ? `${hotkeyStatus.activeShortcut} remains active.` : "Open Quick Launch from the sidebar instead."}{hotkeyStatus.errorCode ? ` (${hotkeyStatus.errorCode})` : ""}</div>}
      {launchError && <div className="quick-launch-warning launch-error" role="alert">{launchError}</div>}
      <div className="quick-launch-results" role="listbox" aria-label="Search results" aria-busy={loading}>
        {loading && <div className="quick-launch-state">Searching your workspace…</div>}
        {!loading && error && <div className="quick-launch-state error" role="alert">{error}</div>}
        {!loading && !error && entryCount === 0 && <div className="quick-launch-state">No matching resources or saved shortcuts. Try another name, tag, or location.</div>}
        {!loading && shortcutEntries.map(({ shortcut, resource, target }, index) => <button key={`shortcut-${shortcut.id}`} role="option" aria-label={`Saved shortcut ${shortcut.name} opens ${resource.displayName}`} aria-selected={index === selectedIndex} className={`quick-launch-result ${index === selectedIndex ? "active" : ""}`} disabled={launching} onMouseEnter={() => setSelectedIndex(index)} onClick={() => void launchShortcut(shortcut)}>
          <span className="quick-launch-shortcut-mark" aria-hidden="true">↗</span><span className="quick-launch-copy"><b>{shortcut.name}</b><small>Opens {resource.displayName}{target ? ` · ${target.canonicalUri}` : " · default active target"}</small></span><span className="quick-launch-enter">↵</span>
        </button>)}
        {!loading && results.map((resource, index) => { const entryIndex = shortcutEntries.length + index; return <button key={`resource-${resource.id}`} role="option" aria-selected={entryIndex === selectedIndex} className={`quick-launch-result ${entryIndex === selectedIndex ? "active" : ""}`} disabled={launching} onMouseEnter={() => setSelectedIndex(entryIndex)} onClick={() => void launchSelected(resource)}>
          <span className={`resource-mark ${resource.resourceType === "folder" ? "folder" : resource.resourceType === "url" ? "web" : "app"}`}>{resource.resourceType === "folder" ? "▱" : resource.resourceType === "url" ? "◎" : resource.displayName.slice(0, 1).toUpperCase()}</span><span className="quick-launch-copy"><b>{resource.displayName}</b><small>{resource.targets[0]?.canonicalUri || resource.description || "No location set"}</small></span><span className="quick-launch-enter">↵</span>
        </button>; })}
      </div>
      <footer className="quick-launch-footer"><span><kbd>↑</kbd><kbd>↓</kbd> to navigate</span><span>{launching ? "Opening…" : <><kbd>↵</kbd> to open</>}</span><span>Search stays on this device</span></footer>
    </section>
  </div>;
}
