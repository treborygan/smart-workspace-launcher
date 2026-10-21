import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import type { ThemePreference } from "../lib/profileTheme";

export function ProfileThemeSettings({ value, onSave }: { value: ThemePreference; onSave: (theme: ThemePreference) => Promise<void> }) {
  const [draft, setDraft] = useState(value);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => { setDraft(value); }, [value]);
  const submit = async (event: FormEvent) => {
    event.preventDefault(); setError(""); setSaving(true);
    try { await onSave(draft); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Could not save appearance settings."); }
    finally { setSaving(false); }
  };
  return <form className="theme-settings" onSubmit={(event) => void submit(event)} aria-label="Appearance settings">
    <h3>Appearance</h3>
    <label>Theme mode<select aria-label="Theme mode" value={draft.mode} onChange={(event) => setDraft((current) => ({ ...current, mode: event.target.value as ThemePreference["mode"] }))}><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></label>
    <label>Custom accent<input aria-label="Custom accent color" type="color" value={draft.accent} onChange={(event) => setDraft((current) => ({ ...current, accent: event.target.value }))} /></label>
    <label>Density<select aria-label="Density" value={draft.density} onChange={(event) => setDraft((current) => ({ ...current, density: event.target.value as ThemePreference["density"] }))}><option value="compact">Compact</option><option value="balanced">Balanced</option><option value="comfortable">Comfortable</option></select></label>
    {error && <p role="alert" className="form-error">{error}</p>}
    <button className="button secondary" disabled={saving}>{saving ? "Saving…" : "Save appearance"}</button>
    <small>Appearance is stored with the selected profile and included in profile export.</small>
  </form>;
}
