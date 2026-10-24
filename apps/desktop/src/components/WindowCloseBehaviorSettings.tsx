import { useEffect, useState } from "react";
import type { ChangeEvent } from "react";
import { api } from "../lib/api";
import type { WindowCloseBehavior } from "../lib/contracts";

export function WindowCloseBehaviorSettings() {
  const [behavior, setBehavior] = useState<WindowCloseBehavior | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    let active = true;
    void api.getWindowCloseBehavior().then((current) => {
      if (active) setBehavior(current);
    }).catch((cause: unknown) => {
      if (active) setError(cause instanceof Error ? cause.message : "Could not load window close behavior.");
    }).finally(() => {
      if (active) setLoading(false);
    });
    return () => { active = false; };
  }, []);

  const changeBehavior = async (event: ChangeEvent<HTMLInputElement>) => {
    const requested = event.currentTarget.value as WindowCloseBehavior;
    if (requested === behavior || saving) return;
    setSaving(true);
    setError("");
    try {
      const saved = await api.setWindowCloseBehavior(requested);
      setBehavior(saved);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not save window close behavior.");
    } finally {
      setSaving(false);
    }
  };

  return <fieldset className="window-close-settings" disabled={loading || saving} aria-busy={loading || saving}>
    <legend>When closing a launcher window</legend>
    <label><input type="radio" name="window-close-behavior" value="hide_to_tray" checked={behavior === "hide_to_tray"} onChange={(event) => void changeBehavior(event)} /> Hide to tray (keep running)</label>
    <label><input type="radio" name="window-close-behavior" value="exit" checked={behavior === "exit"} onChange={(event) => void changeBehavior(event)} /> Exit the app</label>
    <small>This app-wide choice applies to Dashboard, Quick Launch, and Dock close actions. Explicit Hide and Escape controls only hide the current window.</small>
    {loading && <small role="status">Loading close behavior…</small>}
    {error && <p role="alert" className="form-error">{error}</p>}
  </fieldset>;
}
