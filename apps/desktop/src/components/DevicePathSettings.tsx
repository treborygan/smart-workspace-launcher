import { useCallback, useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import type { DevicePathMapping, DevicePathMappingPreview, LocalDeviceIdentity } from "@sw/contracts";
import { api } from "../lib/api";

export function DevicePathSettings() {
  const [device, setDevice] = useState<LocalDeviceIdentity | null>(null);
  const [mappings, setMappings] = useState<DevicePathMapping[]>([]);
  const [name, setName] = useState("");
  const [sourcePrefix, setSourcePrefix] = useState("");
  const [targetPrefix, setTargetPrefix] = useState("");
  const [preview, setPreview] = useState<DevicePathMappingPreview | null>(null);
  const [pendingDelete, setPendingDelete] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const previewRequestRevision = useRef(0);
  const keepDeleteFocus = useRef<HTMLButtonElement | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const [identity, rules] = await Promise.all([api.localDevice(), api.devicePathMappings()]);
      setDevice(identity);
      setName(identity.name);
      setMappings(rules);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not load device path settings.");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);
  useEffect(() => { if (pendingDelete) keepDeleteFocus.current?.focus(); }, [pendingDelete]);

  const saveName = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true); setError(""); setMessage("");
    try {
      const updated = await api.renameLocalDevice(name.trim());
      setDevice(updated); setName(updated.name); setMessage("Device label saved.");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not save the device label.");
    } finally { setBusy(false); }
  };

  const createPreview = async (event: FormEvent) => {
    event.preventDefault();
    const revision = ++previewRequestRevision.current;
    const request = { sourcePrefix: sourcePrefix.trim(), targetPrefix: targetPrefix.trim() };
    setBusy(true); setPreview(null); setError(""); setMessage("");
    try {
      const result = await api.previewDevicePathMapping(request);
      if (revision === previewRequestRevision.current
        && result.sourcePrefix === request.sourcePrefix
        && result.targetPrefix === request.targetPrefix) setPreview(result);
    } catch (reason) {
      if (revision === previewRequestRevision.current) setError(reason instanceof Error ? reason.message : "Could not preview this path mapping.");
    } finally { setBusy(false); }
  };

  const commitPreview = async () => {
    if (!preview) return;
    setBusy(true); setError(""); setMessage("");
    try {
      await api.commitDevicePathMapping(preview);
      setPreview(null); setSourcePrefix(""); setTargetPrefix("");
      await refresh(); setMessage("Device path mapping approved and saved.");
    } catch (reason) {
      setPreview(null);
      setError(reason instanceof Error ? reason.message : "This preview could not be applied. Create a fresh preview and review it again.");
    } finally { setBusy(false); }
  };

  const removeMapping = async (id: string) => {
    setBusy(true); setError(""); setMessage("");
    try {
      await api.deleteDevicePathMapping(id);
      setMappings((current) => current.filter((item) => item.id !== id));
      setPendingDelete(null); setMessage("Device path mapping removed.");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not remove this mapping.");
    } finally { setBusy(false); }
  };

  return <section className="device-path-settings" aria-labelledby="device-path-title">
    <div className="welcome-row"><div><h1 id="device-path-title">Device paths</h1><p>Resolve imported workspace paths on this installation without changing saved resource targets.</p></div><button className="button secondary" type="button" onClick={() => void refresh()} disabled={loading || busy}>Refresh</button></div>
    {error && <p className="device-path-alert error" role="alert">{error}</p>}
    {message && <p className="device-path-alert success" role="status">{message}</p>}
    {loading ? <p className="loading-state" role="status">Loading local device settings…</p> : <>
      {device && <section className="device-path-card" aria-labelledby="local-device-heading">
        <header><div><h2 id="local-device-heading">This installation</h2><p>A random local ID keeps mappings separate from other devices. No hardware identifier is collected.</p></div></header>
        <form className="device-path-device-form" onSubmit={(event) => void saveName(event)}>
          <label htmlFor="local-device-name">Device label</label>
          <input id="local-device-name" value={name} maxLength={60} onChange={(event) => setName(event.target.value)} aria-describedby="local-device-id" />
          <button className="button secondary" type="submit" disabled={busy || !name.trim() || name.trim() === device.name}>Save label</button>
          <small id="local-device-id">Local ID: <code>{device.id}</code></small>
        </form>
      </section>}

      <div className="device-path-grid">
        <section className="device-path-card" aria-labelledby="mapping-create-heading">
          <header><div><h2 id="mapping-create-heading">Add a path mapping</h2><p>Preview exact file and folder targets before approving a rule for this device.</p></div></header>
          <form className="device-path-create" onSubmit={(event) => void createPreview(event)}>
            <label htmlFor="mapping-source">Saved path prefix<input id="mapping-source" value={sourcePrefix} onChange={(event) => { ++previewRequestRevision.current; setSourcePrefix(event.target.value); setPreview(null); }} disabled={busy} placeholder={"C:\\Shared\\Projects or \\\\server\\share\\Projects"} autoCapitalize="off" spellCheck={false} /></label>
            <label htmlFor="mapping-target">This device path<input id="mapping-target" value={targetPrefix} onChange={(event) => { ++previewRequestRevision.current; setTargetPrefix(event.target.value); setPreview(null); }} disabled={busy} placeholder={"D:\\Work\\Projects or \\\\server\\share\\Projects"} autoCapitalize="off" spellCheck={false} /></label>
            <p className="device-path-note">Use an absolute drive-letter or UNC path. File URIs and device namespace paths are not accepted. Only active file and folder targets are considered. Prefixes match on folder boundaries; Windows matching ignores letter case. Saved target paths remain unchanged.</p>
            <button className="button primary" type="submit" disabled={busy || !sourcePrefix.trim() || !targetPrefix.trim()}>Preview mapping</button>
          </form>
          {preview && <section className="device-path-preview" aria-labelledby="mapping-preview-heading">
            <header><div><h3 id="mapping-preview-heading">Review before approval</h3><p>{preview.changes.length} current {preview.changes.length === 1 ? "target" : "targets"} will resolve through this rule on this device.</p><p className="device-path-preview-rule"><code>{preview.sourcePrefix}</code><span aria-hidden="true">→</span><code>{preview.targetPrefix}</code></p></div></header>
            {preview.changes.length > 0 ? <ul className="device-path-changes">{preview.changes.map((change) => <li key={`${change.resourceId}:${change.targetId}`}><code>{change.fromUri}</code><span aria-hidden="true">→</span><code>{change.toUri}</code></li>)}</ul> : <p className="device-path-note">No current targets match. You can still save this rule for resources imported later.</p>}
            <div className="device-path-actions"><button className="button secondary" type="button" disabled={busy} onClick={() => setPreview(null)}>Cancel preview</button><button className="button primary" type="button" disabled={busy} onClick={() => void commitPreview()}>{busy ? "Saving…" : "Approve and save mapping"}</button></div>
          </section>}
        </section>

        <section className="device-path-card" aria-labelledby="mapping-list-heading">
          <header><div><h2 id="mapping-list-heading">Approved mappings</h2><p>Rules are local to this installation and apply only during launch resolution.</p></div><span className="device-path-count">{mappings.length} / 100</span></header>
          {mappings.length === 0 ? <p className="empty-state device-path-empty">No approved path mappings. Import a profile from another device, then add a rule here if its paths differ.</p> : <ul className="device-path-list">{mappings.map((mapping) => <li key={mapping.id}>
            <div className="device-path-rule"><code>{mapping.sourcePrefix}</code><span aria-hidden="true">→</span><code>{mapping.targetPrefix}</code><small>Approved for {device?.name || "this device"}</small></div>
            {pendingDelete === mapping.id ? <div className="device-path-actions"><span className="device-path-confirm" role="status" aria-live="polite">Remove this rule?</span><button ref={keepDeleteFocus} className="button secondary" type="button" onClick={() => setPendingDelete(null)}>Keep</button><button className="button danger" type="button" disabled={busy} onClick={() => void removeMapping(mapping.id)}>Remove</button></div> : <button className="button danger" type="button" aria-label={`Remove mapping ${mapping.sourcePrefix} to ${mapping.targetPrefix}`} disabled={busy} onClick={() => setPendingDelete(mapping.id)}>Remove</button>}
          </li>)}</ul>}
        </section>
      </div>
      <p className="device-path-portability">Profile exports keep the original resource paths and omit local device mappings. Each destination device needs its own reviewed mapping.</p>
    </>}
  </section>;
}
