import { useState } from "react";
import type { Profile, ProfileExportResult, ProfileImportPreview, ProfileImportResult, ProfileImportStrategy } from "@sw/contracts";
import { api } from "../lib/api";
import { chooseProfileExportPath, chooseProfilePackage } from "../lib/profileDialogs";

export function ProfileArchiveControls({ profiles, activeProfileId, onImported }: { profiles: Profile[]; activeProfileId: string; onImported: (profileId: string) => void }) {
  const [includeHistory, setIncludeHistory] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [result, setResult] = useState("");
  const [packagePath, setPackagePath] = useState("");
  const [preview, setPreview] = useState<ProfileImportPreview | null>(null);
  const [strategy, setStrategy] = useState<ProfileImportStrategy>("duplicate");
  const [targetProfileId, setTargetProfileId] = useState(activeProfileId);
  const [confirming, setConfirming] = useState(false);
  const [importResult, setImportResult] = useState<ProfileImportResult | null>(null);
  const profile = profiles.find((item) => item.id === activeProfileId);
  const desktopAvailable = api.isNativeApp();
  const refreshPreview = async (nextStrategy: ProfileImportStrategy, nextTargetProfileId: string) => {
    if (!packagePath) return;
    setBusy(true); setError(""); setConfirming(false);
    try {
      const nextPreview = await api.previewProfileImport(packagePath, nextStrategy === "duplicate" ? undefined : nextTargetProfileId);
      setPreview(nextPreview);
    } catch (reason) { setPreview(null); setError(message(reason, "Could not refresh the import preview.")); }
    finally { setBusy(false); }
  };
  const exportProfile = async () => {
    if (!profile) return;
    setBusy(true); setError(""); setResult("");
    try {
      const outputPath = await chooseProfileExportPath(profile.name);
      if (!outputPath) return;
      const exported: ProfileExportResult = await api.exportProfile(profile.id, outputPath, includeHistory);
      setResult(`Exported ${exported.resourceCount} resources from ${exported.profileName} to ${exported.path}${includeHistory ? ` with ${exported.historyEventCount} history events` : ""}.`);
    } catch (reason) { setError(message(reason, "Profile export failed.")); }
    finally { setBusy(false); }
  };
  const inspectPackage = async () => {
    setBusy(true); setError(""); setResult(""); setPreview(null); setImportResult(null); setConfirming(false);
    try {
      const path = await chooseProfilePackage();
      if (!path) return;
      const data = await api.previewProfileImport(path);
      setPackagePath(path); setPreview(data); setTargetProfileId(activeProfileId);
      setStrategy(data.profileIdCollision || data.profileNameCollision ? "duplicate" : "duplicate");
    } catch (reason) { setError(message(reason, "Could not validate this profile package.")); }
    finally { setBusy(false); }
  };
  const commitImport = async () => {
    if (!preview || !packagePath) return;
    setBusy(true); setError("");
    try {
      const commitTargetId = strategy === "duplicate" ? undefined : targetProfileId;
      const refreshed = await api.previewProfileImport(packagePath, commitTargetId);
      if (refreshed.packageSha256 !== preview.packageSha256 || refreshed.previewToken !== preview.previewToken) {
        setPreview(refreshed); setConfirming(false); setError("The import preview changed. Review the updated collisions and confirm again.");
        return;
      }
      const imported = await api.commitProfileImport(packagePath, strategy, commitTargetId, refreshed.packageSha256, refreshed.previewToken);
      setImportResult(imported); setConfirming(false); setResult(`Imported ${imported.importedResources} resources into ${imported.profileName}. ${imported.reusedResources} existing resources reused; ${imported.importedLayouts} layouts and ${imported.importedHistoryEvents} history events imported.`);
      onImported(imported.profileId);
    } catch (reason) { setError(message(reason, "Profile import failed. No changes were applied.")); }
    finally { setBusy(false); }
  };
  return <section className="profile-archive" aria-label="Profile backup and restore">
    <h3>Profile backup</h3>
    <p>Export or safely import a workspace profile package.</p>
    {!desktopAvailable && <p className="archive-security">Profile package dialogs are available in the Windows desktop app.</p>}
    <label className="archive-history"><input type="checkbox" checked={includeHistory} onChange={(event) => setIncludeHistory(event.target.checked)} /> Include app-recorded launch history</label>
    <button className="button secondary" onClick={() => void exportProfile()} disabled={!desktopAvailable || busy || !profile}>{busy ? "Working…" : "Export profile…"}</button>
    <button className="button secondary" onClick={() => void inspectPackage()} disabled={!desktopAvailable || busy}>{busy ? "Working…" : "Import profile…"}</button>
    <p className="archive-security">Packages are validated and previewed before import. Import never runs scripts or executable content.</p>
    {error && <p className="archive-error" role="alert">{error}</p>}
    {result && <p className="archive-result" role="status">{result}</p>}
    {preview && !importResult && <div className="archive-preview" aria-label="Import preview">
      <h4>Review import · {preview.profileName}</h4>
      <p>This preview is read-only. No data has changed.</p>
      <details className="archive-checksum"><summary>Reviewed package checksum</summary><code>{preview.packageSha256}</code></details>
      <dl><div><dt>Resources</dt><dd>{preview.resourceCount}</dd></div><div><dt>Layouts</dt><dd>{preview.layoutCount}</dd></div><div><dt>Smart collections</dt><dd>{preview.smartCollectionCount}</dd></div><div><dt>Quick Launch shortcuts</dt><dd>{preview.dynamicShortcutCount}</dd></div><div><dt>History events</dt><dd>{preview.historyEventCount}</dd></div><div><dt>Assets</dt><dd>{preview.assetCount}</dd></div></dl>
      <div className="archive-collisions"><b>Collisions {preview.targetProfileId ? `in ${profiles.find((item) => item.id === preview.targetProfileId)?.name || "the selected target profile"}` : "for a new duplicate profile"}</b><span>Profile ID: {preview.profileIdCollision ? "yes" : "no"}</span><span>Profile name: {preview.profileNameCollision ? "yes" : "no"}</span><span>Resource IDs: {preview.resourceIdCollisions.length}</span><span>Target IDs: {preview.targetIdCollisions.length}</span><span>Smart collection IDs/names: {preview.smartCollectionIdCollisions.length}/{preview.smartCollectionNameCollisions.length}</span><span>Shortcut IDs/names: {preview.dynamicShortcutIdCollisions.length}/{preview.dynamicShortcutNameCollisions.length}</span></div>
      {(preview.resourceIdCollisions.length > 0 || preview.targetIdCollisions.length > 0 || preview.smartCollectionIdCollisions.length > 0 || preview.smartCollectionNameCollisions.length > 0 || preview.dynamicShortcutIdCollisions.length > 0 || preview.dynamicShortcutNameCollisions.length > 0) && <details><summary>Collision details</summary><p>Resource IDs: {preview.resourceIdCollisions.join(", ") || "none"}</p><p>Target IDs: {preview.targetIdCollisions.join(", ") || "none"}</p><p>Smart collection IDs: {preview.smartCollectionIdCollisions.join(", ") || "none"}</p><p>Smart collection names: {preview.smartCollectionNameCollisions.join(", ") || "none"}</p><p>Quick Launch shortcut IDs: {preview.dynamicShortcutIdCollisions.join(", ") || "none"}</p><p>Quick Launch shortcut names: {preview.dynamicShortcutNameCollisions.join(", ") || "none"}</p></details>}
      {!!preview.resourceConflicts?.length && <details className="archive-conflict-list" open><summary>Resource value review · {preview.resourceConflicts.length}</summary><p>Each conflict shows package values, existing local values, and the content the selected strategy will apply. IDs may be remapped during import.</p>{preview.resourceConflicts.map((conflict, index) => {
        const proposed = strategy === "merge" ? conflict.proposed : safeImportSnapshot(conflict.imported);
        const resolution = strategy === "merge" ? conflict.resolution === "reuse_existing" ? "Merge will reuse the matching existing resource" : "Merge will import as a new resource with remapped IDs" : strategy === "replace" ? "Replace clears profile membership, then imports this as a new resource" : "Duplicate creates this as a new resource with remapped IDs";
        return <article className="archive-conflict" key={`${conflict.imported.id}-${index}`}><h5>{conflict.imported.name} <small>{resolution}</small></h5><div className="archive-snapshot-grid"><Snapshot title="Package" resource={conflict.imported} /><Snapshot title="Existing" resource={conflict.existing} /><Snapshot title={strategy === "merge" ? "Proposed result" : "Imported values"} resource={proposed} /></div></article>;
      })}</details>}
      <label>Import strategy<select value={strategy} disabled={busy} onChange={(event) => { const next = event.target.value as ProfileImportStrategy; setStrategy(next); void refreshPreview(next, targetProfileId); }}><option value="duplicate">Duplicate as a new profile</option><option value="merge">Merge into an existing profile</option><option value="replace">Replace an existing profile’s contents</option></select></label>
      <p className="archive-security">Duplicate keeps the package as a separate profile. Merge reuses matching resources and retains both collection/shortcut names by remapping collisions. Replace replaces the selected profile’s contents.</p>
      {strategy !== "duplicate" && <label>Target profile<select value={targetProfileId} disabled={busy} onChange={(event) => { const next = event.target.value; setTargetProfileId(next); void refreshPreview(strategy, next); }}>{profiles.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}</select></label>}
      {!confirming ? <button className="button primary" onClick={() => setConfirming(true)} disabled={busy || (strategy !== "duplicate" && !targetProfileId)}>Review commit…</button> : <div className="archive-confirm" role="group" aria-label="Confirm profile import"><b>{strategy === "replace" ? "Replace profile contents?" : strategy === "merge" ? "Merge this profile?" : "Create a duplicate profile?"}</b><span>{strategy === "replace" ? "Existing resources and layouts for the selected profile will be replaced." : strategy === "merge" ? "Matching resources will be reused and package content merged." : "A new profile will be created from this package."}</span><button className="button primary" onClick={() => void commitImport()} disabled={busy}>{busy ? "Importing…" : "Confirm import"}</button><button className="button secondary" onClick={() => setConfirming(false)} disabled={busy}>Cancel</button></div>}
    </div>}
  </section>;
}
function message(reason: unknown, fallback: string) { return reason instanceof Error ? reason.message : fallback; }
function Snapshot({ title, resource }: { title: string; resource: import("@sw/contracts").ProfileResourceSnapshot }) {
  return <section><b>{title}</b><span>{resource.name} · {resource.resourceType}</span><span>{resource.description || "No description"}</span><span>Tags: {resource.tags.length ? resource.tags.join(", ") : "none"}</span><ul>{resource.targets.map((target) => <li key={target.id}>{target.kind}: {target.value}{target.active ? " · active" : " · inactive"}{target.arguments.length ? ` · args: ${target.arguments.join(" ")}` : ""}</li>)}</ul></section>;
}
function safeImportSnapshot(resource: import("@sw/contracts").ProfileResourceSnapshot): import("@sw/contracts").ProfileResourceSnapshot {
  return { ...resource, targets: resource.targets.map((target) => ({ ...target, active: target.active && target.kind !== "app" && target.kind !== "shortcut" })) };
}
