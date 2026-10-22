import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import type { ExtensionCapability, ExtensionManifest, RegisteredExtension } from "@sw/contracts";
import { api } from "../lib/api";

const CAPABILITIES: { value: ExtensionCapability; label: string; detail: string }[] = [
  { value: "resource_metadata.read", label: "Read resource metadata", detail: "Names and launcher metadata for saved resources." },
  { value: "workspace_summary.read", label: "Read workspace summary", detail: "A summary of the current workspace." },
];

const emptyManifest = (): ExtensionManifest => ({ id: "", name: "", version: "1.0.0", sdkVersion: "1.0.0", description: "", requestedCapabilities: [] });

export function ExtensionsRegistry() {
  const native = api.isNativeApp();
  const [extensions, setExtensions] = useState<RegisteredExtension[]>([]);
  const [manifest, setManifest] = useState<ExtensionManifest>(emptyManifest);
  const [loading, setLoading] = useState(native);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [pendingRemove, setPendingRemove] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const keepRemoveFocus = useRef<HTMLButtonElement | null>(null);

  const refresh = async () => {
    if (!native) return;
    setLoading(true); setError("");
    try { setExtensions(await api.extensionRegistry()); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Could not load the extension registry."); }
    finally { setLoading(false); }
  };

  useEffect(() => { void refresh(); }, []);
  useEffect(() => { if (pendingRemove) keepRemoveFocus.current?.focus(); }, [pendingRemove]);

  const updateManifest = <K extends keyof ExtensionManifest>(key: K, value: ExtensionManifest[K]) => setManifest((current) => ({ ...current, [key]: value }));

  const register = async (event: FormEvent) => {
    event.preventDefault(); setError(""); setMessage(""); setSaving(true);
    try {
      const registered = await api.registerExtension({
        ...manifest,
        id: manifest.id.trim(), name: manifest.name.trim(), description: manifest.description.trim(),
      });
      setExtensions((current) => [...current, registered].sort((a, b) => a.name.localeCompare(b.name)));
      setManifest(emptyManifest()); setMessage(`${registered.name} was added as a disabled registry entry.`);
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Could not add this extension manifest."); }
    finally { setSaving(false); }
  };

  const changeGrants = async (extension: RegisteredExtension, capability: ExtensionCapability, checked: boolean) => {
    const grantedCapabilities = checked
      ? [...new Set([...extension.grantedCapabilities, capability])]
      : extension.grantedCapabilities.filter((item) => item !== capability);
    setBusyId(extension.id); setError(""); setMessage("");
    try {
      const updated = await api.setExtensionPermissions({ extensionId: extension.id, manifestVersion: extension.version, grantedCapabilities });
      setExtensions((current) => current.map((item) => item.id === updated.id ? updated : item));
      setMessage(`Saved capability choices for ${updated.name}. These grants have no effect until extension execution is implemented.`);
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Could not save capability choices."); }
    finally { setBusyId(null); }
  };

  const changeEnabled = async (extension: RegisteredExtension, enabled: boolean) => {
    setBusyId(extension.id); setError(""); setMessage("");
    try {
      const updated = await api.setExtensionEnabled(extension.id, enabled);
      setExtensions((current) => current.map((item) => item.id === updated.id ? updated : item));
      setMessage(`${updated.name} is ${enabled ? "enabled" : "disabled"} in the registry. Extension code is not executed by this app.`);
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Could not update registry status."); }
    finally { setBusyId(null); }
  };

  const remove = async (extensionId: string) => {
    const extension = extensions.find((item) => item.id === extensionId);
    setBusyId(extensionId); setError(""); setMessage("");
    try {
      await api.removeExtension(extensionId);
      setExtensions((current) => current.filter((item) => item.id !== extensionId));
      setPendingRemove(null); setMessage(`${extension?.name || "Extension"} and its saved grants were removed.`);
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Could not remove this extension."); }
    finally { setBusyId(null); }
  };

  return <section className="extensions-registry" aria-labelledby="extensions-title">
    <div className="welcome-row"><div><h1 id="extensions-title">Extensions</h1><p>Review extension manifests and save capability choices for this installation.</p></div><button type="button" className="button secondary" onClick={() => void refresh()} disabled={!native || loading || saving || busyId !== null}>Refresh</button></div>
    <aside className="extensions-runtime-notice" aria-label="Extension execution status">
      <strong>Registry only — extension execution is unavailable.</strong>
      <p>This page stores extension metadata and your requested/granted capability policy. It does not load packages or run extension code. Saved grants currently have no effect. No file, network, process, launch, shell, or secret access is provided.</p>
    </aside>
    {error && <p className="extensions-alert error" role="alert">{error}</p>}
    {message && <p className="extensions-alert success" role="status">{message}</p>}
    {!native ? <p className="extensions-empty" role="status">The extension registry is available in the native desktop app only.</p> : <>
      <div className="extensions-layout">
        <section className="extensions-card" aria-labelledby="extension-add-title">
          <header><div><h2 id="extension-add-title">Register a manifest</h2><p>Add metadata only. Do not paste code or executable content.</p></div></header>
          <form className="extension-form" onSubmit={(event) => void register(event)}>
            <label htmlFor="extension-id">Extension ID<input id="extension-id" required maxLength={255} value={manifest.id} onChange={(event) => updateManifest("id", event.target.value)} placeholder="com.example.focus-summary" autoCapitalize="off" spellCheck={false} /></label>
            <label htmlFor="extension-name">Name<input id="extension-name" required minLength={1} maxLength={80} value={manifest.name} onChange={(event) => updateManifest("name", event.target.value)} /></label>
            <label htmlFor="extension-version">Version<input id="extension-version" required maxLength={32} value={manifest.version} onChange={(event) => updateManifest("version", event.target.value)} aria-describedby="extension-version-help" /></label>
            <small id="extension-version-help" className="extension-help">Use MAJOR.MINOR.PATCH, for example 1.0.0.</small>
            <label htmlFor="extension-description">Description<textarea id="extension-description" required maxLength={240} rows={3} value={manifest.description} onChange={(event) => updateManifest("description", event.target.value)} /></label>
            <fieldset className="extension-capability-picker">
              <legend>Requested capabilities</legend>
              {CAPABILITIES.map((capability) => <label key={capability.value}><input type="checkbox" checked={manifest.requestedCapabilities.includes(capability.value)} onChange={(event) => updateManifest("requestedCapabilities", event.target.checked ? [...manifest.requestedCapabilities, capability.value] : manifest.requestedCapabilities.filter((item) => item !== capability.value))} /><span><b>{capability.label}</b><small>{capability.detail}</small></span></label>)}
            </fieldset>
            <p className="extension-help">The SDK version is fixed at 1.0.0. Up to two capabilities may be requested. Registration never enables an entry or grants permissions.</p>
            <button className="button primary" type="submit" disabled={saving || loading}>{saving ? "Registering…" : "Register disabled entry"}</button>
          </form>
        </section>

        <section className="extensions-card" aria-labelledby="extension-list-title">
          <header><div><h2 id="extension-list-title">Registered extensions</h2><p>Grant choices are limited to the capabilities declared in each manifest.</p></div></header>
          {loading ? <p className="extensions-empty" role="status">Loading registry…</p> : extensions.length === 0 ? <p className="extensions-empty">No extension manifests registered.</p> : <ul className="extension-list">{extensions.map((extension) => {
            const busy = busyId === extension.id;
            const removing = pendingRemove === extension.id;
            return <li className="extension-item" key={extension.id}>
              <header><div><h3>{extension.name}</h3><p><code>{extension.id}</code> · v{extension.version}</p><p>{extension.description}</p></div><span className={`extension-status ${extension.enabled ? "enabled" : "disabled"}`}>{extension.enabled ? "Enabled" : "Disabled"}</span></header>
              <fieldset className="extension-grants" disabled={busy}>
                <legend>Requested / granted capabilities</legend>
                {extension.requestedCapabilities.length === 0 ? <p className="extension-help">No capabilities requested.</p> : extension.requestedCapabilities.map((capability) => {
                  const definition = CAPABILITIES.find((item) => item.value === capability);
                  return <label key={capability}><input type="checkbox" checked={extension.grantedCapabilities.includes(capability)} onChange={(event) => void changeGrants(extension, capability, event.target.checked)} /><span><b>{definition?.label || capability}</b><small>{extension.grantedCapabilities.includes(capability) ? "Granted in saved policy" : "Not granted"} · requested by manifest</small></span></label>;
                })}
              </fieldset>
              <div className="extension-controls">
                <label className="extension-enable"><input type="checkbox" checked={extension.enabled} disabled={busy} onChange={(event) => void changeEnabled(extension, event.target.checked)} /><span>Enable registry entry</span></label>
                {!removing ? <button type="button" className="button secondary danger" disabled={busy} onClick={() => setPendingRemove(extension.id)}>Remove</button> : <div className="extension-confirm" role="group" aria-label={`Remove ${extension.name} confirmation`}><span>Remove this entry and its grants?</span><button type="button" className="button secondary" ref={keepRemoveFocus} disabled={busy} onClick={() => setPendingRemove(null)}>Keep</button><button type="button" className="button secondary danger" disabled={busy} onClick={() => void remove(extension.id)}>Remove</button></div>}
              </div>
            </li>;
          })}</ul>}
        </section>
      </div>
    </>}
  </section>;
}
