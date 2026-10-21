import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { Icon } from "./Icon";
import { api } from "../lib/api";
import type { Resource } from "../lib/contracts";
import type { QuickLaunchHotkeyStatus } from "@sw/contracts";

type Props = {
  resources: Resource[];
  profileId?: string;
  hotkeyStatus: QuickLaunchHotkeyStatus | null;
  presentation?: "overlay" | "window";
  onClose: () => void;
  onLaunch: (resource: Resource) => void;
};

export function QuickLaunchOverlay({ resources, profileId, hotkeyStatus, presentation = "overlay", onClose, onLaunch }: Props) {
  const inputRef = useRef<HTMLInputElement>(null);
  const requestRef = useRef(0);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Resource[]>(resources);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

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

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") { event.preventDefault(); setSelectedIndex((index) => Math.min(index + 1, Math.max(0, results.length - 1))); }
    if (event.key === "ArrowUp") { event.preventDefault(); setSelectedIndex((index) => Math.max(0, index - 1)); }
    if (event.key === "Enter" && results[selectedIndex]) { event.preventDefault(); onLaunch(results[selectedIndex]); onClose(); }
    if (event.key === "Escape") { event.preventDefault(); onClose(); }
  };

  return <div className={presentation === "window" ? "quick-launch-window" : "quick-launch-backdrop"} role="presentation" onMouseDown={(event) => presentation === "overlay" && event.target === event.currentTarget && onClose()}>
    <section className={`quick-launch ${presentation === "window" ? "in-window" : ""}`} role="dialog" aria-modal="true" aria-label="Quick Launch">
      <div className="quick-launch-search"><Icon name="search" size={20} /><input ref={inputRef} aria-label="Search resources" placeholder="Search resources, aliases, tags…" value={query} onChange={(event) => setQuery(event.target.value)} onKeyDown={handleKeyDown} /><kbd>ESC</kbd></div>
      {hotkeyStatus && !hotkeyStatus.registered && <div className="quick-launch-warning" role="status">{hotkeyStatus.shortcut} could not be registered. Open Quick Launch from the sidebar instead.{hotkeyStatus.errorCode ? ` (${hotkeyStatus.errorCode})` : ""}</div>}
      <div className="quick-launch-results" role="listbox" aria-label="Search results" aria-busy={loading}>
        {loading && <div className="quick-launch-state">Searching your workspace…</div>}
        {!loading && error && <div className="quick-launch-state error" role="alert">{error}</div>}
        {!loading && !error && results.length === 0 && <div className="quick-launch-state">No matching resources. Try another name, tag, or location.</div>}
        {!loading && !error && results.map((resource, index) => <button key={resource.id} role="option" aria-selected={index === selectedIndex} className={`quick-launch-result ${index === selectedIndex ? "active" : ""}`} onMouseEnter={() => setSelectedIndex(index)} onClick={() => { onLaunch(resource); onClose(); }}>
          <span className={`resource-mark ${resource.resourceType === "folder" ? "folder" : resource.resourceType === "url" ? "web" : "app"}`}>{resource.resourceType === "folder" ? "▱" : resource.resourceType === "url" ? "◎" : resource.displayName.slice(0, 1).toUpperCase()}</span><span className="quick-launch-copy"><b>{resource.displayName}</b><small>{resource.targets[0]?.canonicalUri || resource.description || "No location set"}</small></span><span className="quick-launch-enter">↵</span>
        </button>)}
      </div>
      <footer className="quick-launch-footer"><span><kbd>↑</kbd><kbd>↓</kbd> to navigate</span><span><kbd>↵</kbd> to open</span><span>Search stays on this device</span></footer>
    </section>
  </div>;
}
