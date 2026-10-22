import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { ExtensionCapability, ExtensionPackagePreview, ExtensionRunResult, RegisteredExtension } from "@sw/contracts";
import { api } from "../lib/api";

const CAPABILITIES: { value: ExtensionCapability; label: string; detail: string }[] = [
  { value: "resource_metadata.read", label: "Read resource metadata", detail: "Names, types, categories, tags and favorite state for up to 200 active-profile resources. No paths or file contents." },
  { value: "workspace_summary.read", label: "Read workspace summary", detail: "Aggregate resource and favorite counts for the active profile." },
];

type ReviewedPackage = { path: string; preview: ExtensionPackagePreview };
type RunOutput = { extensionId: string; profileId: string; result: ExtensionRunResult };

export function ExtensionsRegistry({ profileId }: { profileId: string }) {
  const native = api.isNativeApp();
  const [extensions, setExtensions] = useState<RegisteredExtension[]>([]);
  const [reviewed, setReviewed] = useState<ReviewedPackage | null>(null);
  const [loading, setLoading] = useState(native);
  const [busy, setBusy] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [pendingRemove, setPendingRemove] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [runOutput, setRunOutput] = useState<RunOutput | null>(null);
  const keepRemoveFocus = useRef<HTMLButtonElement | null>(null);
  const packageOperation = useRef(0);
  const runOperation = useRef(0);

  const refresh = useCallback(async () => {
    if (!native) return;
    setLoading(true); setError("");
    try { setExtensions(await api.extensionRegistry()); }
    catch (reason) { setError(errorMessage(reason, "Could not load the extension registry.")); }
    finally { setLoading(false); }
  }, [native]);

  useEffect(() => { void refresh(); }, [refresh]);
  useEffect(() => { if (pendingRemove) keepRemoveFocus.current?.focus(); }, [pendingRemove]);
  useEffect(() => {
    runOperation.current += 1;
    setBusyId(null); setRunOutput(null); setMessage(""); setError("");
  }, [profileId]);

  const choosePackage = async () => {
    const operation = ++packageOperation.current;
    setError(""); setMessage(""); setReviewed(null); setRunOutput(null);
    if (!native) { setError("Extension package review is available in the native desktop app."); return; }
    setBusy(true);
    try {
      const selected = await open({ multiple: false, directory: false, filters: [{ name: "Smart Workspace extension", extensions: ["swext"] }] });
      if (operation !== packageOperation.current || typeof selected !== "string") return;
      const preview = await api.previewExtensionPackage({ packagePath: selected });
      if (operation !== packageOperation.current) return;
      setReviewed({ path: selected, preview });
      setMessage("Package checked without running its module. Review its requested capabilities before installing.");
    } catch (reason) {
      if (operation === packageOperation.current) {
        setReviewed(null);
        setError(errorMessage(reason, "Could not validate this extension package."));
      }
    } finally { if (operation === packageOperation.current) setBusy(false); }
  };

  const installReviewed = async () => {
    if (!reviewed) return;
    const selected = reviewed;
    const operation = ++packageOperation.current;
    setBusy(true); setError(""); setMessage(""); setRunOutput(null);
    try {
      const installed = await api.installExtensionPackage({ packagePath: selected.path, packageSha256: selected.preview.packageSha256, previewToken: selected.preview.previewToken });
      if (operation !== packageOperation.current) return;
      setExtensions((current) => [...current.filter((item) => item.id !== installed.id), installed].sort((a, b) => a.name.localeCompare(b.name)));
      setReviewed(null);
      setMessage(`${installed.name} installed disabled. Review its capabilities and enable it explicitly before a run.`);
    } catch (reason) {
      if (operation === packageOperation.current) {
        setReviewed(null);
        setError(`${errorMessage(reason, "Extension installation failed.")} Review the package again before retrying.`);
      }
    } finally { if (operation === packageOperation.current) setBusy(false); }
  };

  const changeGrants = async (extension: RegisteredExtension, capability: ExtensionCapability, checked: boolean) => {
    const grantedCapabilities = checked
      ? [...new Set([...extension.grantedCapabilities, capability])]
      : extension.grantedCapabilities.filter((item) => item !== capability);
    setBusyId(extension.id); setError(""); setMessage(""); setRunOutput(null);
    try {
      const updated = await api.setExtensionPermissions({ extensionId: extension.id, manifestVersion: extension.version, grantedCapabilities });
      setExtensions((current) => current.map((item) => item.id === updated.id ? updated : item));
      setMessage(`Saved capability choices for ${updated.name}. A run request sends data only for the capabilities currently granted.`);
    } catch (reason) { setError(errorMessage(reason, "Could not save capability choices.")); }
    finally { setBusyId(null); }
  };

  const changeEnabled = async (extension: RegisteredExtension, enabled: boolean) => {
    setBusyId(extension.id); setError(""); setMessage(""); setRunOutput(null);
    try {
      const updated = await api.setExtensionEnabled(extension.id, enabled);
      setExtensions((current) => current.map((item) => item.id === updated.id ? updated : item));
      setMessage(`${updated.name} is ${enabled ? "enabled" : "disabled"}. It will run only after you explicitly choose Run.`);
    } catch (reason) { setError(errorMessage(reason, "Could not update extension status.")); }
    finally { setBusyId(null); }
  };

  const run = async (extension: RegisteredExtension) => {
    if (!extension.enabled || !extension.moduleSha256 || !profileId) return;
    const operation = ++runOperation.current;
    const requestedProfile = profileId;
    setBusyId(extension.id); setError(""); setMessage(""); setRunOutput(null);
    try {
      const result = await api.runExtension(extension.id, requestedProfile);
      if (operation !== runOperation.current || requestedProfile !== profileId) return;
      setRunOutput({ extensionId: extension.id, profileId: requestedProfile, result });
      setMessage(`${extension.name} returned ${result.suggestions.length} text suggestion${result.suggestions.length === 1 ? "" : "s"}. Nothing was launched or changed.`);
    } catch (reason) {
      if (operation === runOperation.current && requestedProfile === profileId) setError(errorMessage(reason, "The native service could not run this extension. No suggestions were applied."));
    } finally { if (operation === runOperation.current) setBusyId(null); }
  };

  const remove = async (extensionId: string) => {
    const extension = extensions.find((item) => item.id === extensionId);
    setBusyId(extensionId); setError(""); setMessage(""); setRunOutput(null);
    try {
      await api.removeExtension(extensionId);
      setExtensions((current) => current.filter((item) => item.id !== extensionId));
      setPendingRemove(null); setMessage(`${extension?.name || "Extension"}, its module and saved grants were removed.`);
    } catch (reason) { setError(errorMessage(reason, "Could not remove this extension.")); }
    finally { setBusyId(null); }
  };

  return <section className="extensions-registry" aria-labelledby="extensions-title">
    <div className="welcome-row"><div><h1 id="extensions-title">Extensions</h1><p>Review a local package, choose its data permissions, and run it only when you ask.</p></div><div className="extensions-top-actions"><button type="button" className="button secondary" onClick={() => void refresh()} disabled={!native || loading || busy || busyId !== null}>Refresh</button><button type="button" className="button primary" onClick={() => void choosePackage()} disabled={!native || loading || busy || busyId !== null}>{busy ? "Working…" : "Review .swext package…"}</button></div></div>
    <aside className="extensions-runtime-notice" aria-label="Extension execution safety">
      <strong>Explicit sandbox run request</strong>
      <p>Package review never runs a module. Run sends a request to the native service only for an installed, enabled module. The service rejects requests when its sandbox is unavailable. If a sandbox accepts the request, it receives only data for currently granted capabilities and returns bounded suggestions. Suggestions are displayed as plain text only; they never launch resources or change your workspace.</p>
    </aside>
    {error && <p className="extensions-alert error" role="alert">{error}</p>}
    {message && <p className="extensions-alert success" role="status">{message}</p>}
    {!native ? <p className="extensions-empty" role="status">Extension packages and sandbox runs are available in the native desktop app only.</p> : <>
      {reviewed && <section className="extensions-card extension-package-preview" aria-labelledby="extension-preview-title">
        <header><div><h2 id="extension-preview-title">Package preview · {reviewed.preview.manifest.name}</h2><p>No code has run and nothing has been installed.</p></div><button className="button secondary" type="button" disabled={busy} onClick={() => { packageOperation.current += 1; setReviewed(null); setMessage("Package preview discarded."); }}>Discard preview</button></header>
        <dl className="extension-manifest-details"><div><dt>Selected package</dt><dd><code>{reviewed.path}</code></dd></div><div><dt>Extension ID</dt><dd><code>{reviewed.preview.manifest.id}</code></dd></div><div><dt>Version</dt><dd>{reviewed.preview.manifest.version} · SDK {reviewed.preview.manifest.sdkVersion}</dd></div><div><dt>Description</dt><dd>{reviewed.preview.manifest.description}</dd></div><div><dt>Module size</dt><dd>{reviewed.preview.moduleSizeBytes.toLocaleString()} bytes</dd></div><div><dt>Package SHA-256</dt><dd><code>{reviewed.preview.packageSha256}</code></dd></div><div><dt>Module SHA-256</dt><dd><code>{reviewed.preview.moduleSha256}</code></dd></div></dl>
        <div className="extension-preview-capabilities"><h3>Requested capabilities</h3>{reviewed.preview.manifest.requestedCapabilities.length ? <ul>{reviewed.preview.manifest.requestedCapabilities.map((capability) => <li key={capability}><b>{CAPABILITIES.find((item) => item.value === capability)?.label || capability}</b><span>{CAPABILITIES.find((item) => item.value === capability)?.detail}</span></li>)}</ul> : <p>No data capabilities requested.</p>}</div>
        <p className="extension-review-note">Installation stores this module disabled with no capabilities granted. You must approve each grant and enable it separately.</p>
        <button type="button" className="button primary" onClick={() => void installReviewed()} disabled={busy}>{busy ? "Installing…" : "Install disabled package"}</button>
      </section>}

      <section className="extensions-card" aria-labelledby="extension-list-title">
        <header><div><h2 id="extension-list-title">Installed extensions</h2><p>Capabilities are passed as bounded input data; modules have no host filesystem, process, network, clock, random, shell, launch or secret access.</p></div></header>
        {loading ? <p className="extensions-empty" role="status">Loading registry…</p> : extensions.length === 0 ? <p className="extensions-empty">No extension packages installed.</p> : <ul className="extension-list">{extensions.map((extension) => {
          const busyForExtension = busyId !== null;
          const removing = pendingRemove === extension.id;
          const output = runOutput?.extensionId === extension.id && runOutput.profileId === profileId ? runOutput.result : null;
          return <li className="extension-item" key={extension.id}>
            <header><div><h3>{extension.name}</h3><p><code>{extension.id}</code> · v{extension.version} · SDK {extension.sdkVersion}</p><p>{extension.description}</p><p>{extension.moduleSha256 ? <>Module SHA-256: <code>{extension.moduleSha256}</code></> : "No module installed"}</p></div><span className={`extension-status ${extension.enabled ? "enabled" : "disabled"}`}>{extension.enabled ? "Enabled" : "Disabled"}</span></header>
            <fieldset className="extension-grants" disabled={busyForExtension}>
              <legend>Requested / granted capabilities</legend>
              {extension.requestedCapabilities.length === 0 ? <p className="extension-help">No capabilities requested.</p> : extension.requestedCapabilities.map((capability) => {
                const definition = CAPABILITIES.find((item) => item.value === capability);
                return <label key={capability}><input type="checkbox" checked={extension.grantedCapabilities.includes(capability)} onChange={(event) => void changeGrants(extension, capability, event.target.checked)} /><span><b>{definition?.label || capability}</b><small>{definition?.detail} · {extension.grantedCapabilities.includes(capability) ? "Granted" : "Not granted"}</small></span></label>;
              })}
            </fieldset>
            <div className="extension-controls">
              <label className="extension-enable"><input type="checkbox" checked={extension.enabled} disabled={busyForExtension || !extension.moduleSha256} onChange={(event) => void changeEnabled(extension, event.target.checked)} /><span>Enable package</span></label>
              <div className="extension-action-buttons">
                <button type="button" className="button primary" disabled={busyForExtension || !extension.enabled || !extension.moduleSha256} onClick={() => void run(extension)}>{busyForExtension ? "Working…" : "Run"}</button>
                {!removing ? <button type="button" className="button secondary danger" disabled={busyForExtension} onClick={() => setPendingRemove(extension.id)}>Remove</button> : <div className="extension-confirm" role="group" aria-label={`Remove ${extension.name} confirmation`}><span>Remove package and saved grants?</span><button type="button" className="button secondary" ref={keepRemoveFocus} disabled={busyForExtension} onClick={() => setPendingRemove(null)}>Keep</button><button type="button" className="button secondary danger" disabled={busyForExtension} onClick={() => void remove(extension.id)}>Remove</button></div>}
              </div>
            </div>
            {output && <section className="extension-suggestions" aria-labelledby={`suggestions-${extension.id}`}><h4 id={`suggestions-${extension.id}`}>Text suggestions · {output.suggestions.length}</h4>{output.suggestions.length === 0 ? <p>No suggestions were returned.</p> : <ul>{output.suggestions.map((suggestion, index) => <li key={`${index}:${suggestion.title}`}><b>{suggestion.title}</b><p>{suggestion.reason}</p>{suggestion.resourceIds.length > 0 && <p className="extension-resource-refs">Resource IDs: {suggestion.resourceIds.join(", ")}</p>}</li>)}</ul>}<p className="extension-review-note">Suggestions are untrusted text. Review them yourself; no suggestion action has been applied.</p></section>}
          </li>;
        })}</ul>}
      </section>
    </>}
  </section>;
}

function errorMessage(reason: unknown, fallback: string) { return reason instanceof Error ? reason.message : fallback; }
