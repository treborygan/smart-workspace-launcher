import { useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../lib/api";
import type { ResourceImportScanResult, ResourceImportSource } from "@sw/contracts";

type SourceChoice = "directory" | "start_menu" | "bookmarks";

const sourceLabel: Record<SourceChoice, string> = {
  directory: "Directory",
  start_menu: "Start Menu folder",
  bookmarks: "Bookmark HTML file",
};

export function ResourceImportAssistant({ profileId, onImported }: { profileId: string; onImported: (message: string) => void }) {
  const [choice, setChoice] = useState<SourceChoice>("directory");
  const [source, setSource] = useState<ResourceImportSource | null>(null);
  const [scan, setScan] = useState<ResourceImportScanResult | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [duplicatePolicy, setDuplicatePolicy] = useState<"skip_existing" | "create_duplicate">("skip_existing");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [status, setStatus] = useState("");
  const operation = useRef(0);

  useEffect(() => {
    operation.current += 1;
    setBusy(false); setSource(null); setScan(null); setSelected(new Set()); setError(""); setStatus("");
  }, [profileId]);

  const existingCount = useMemo(() => scan?.candidates.filter((candidate) => candidate.existingResourceId).length ?? 0, [scan]);
  const chooseSource = async () => {
    const operationId = ++operation.current;
    const chosenKind = choice;
    setError(""); setStatus(""); setScan(null); setSelected(new Set());
    if (!api.isNativeApp()) { setError("Resource discovery is available in the Windows desktop app."); return; }
    try {
      const picked = await open(chosenKind === "bookmarks"
        ? { multiple: false, directory: false, filters: [{ name: "Bookmarks (HTML)", extensions: ["html", "htm"] }] }
        : { multiple: false, directory: true });
      if (operationId !== operation.current) return;
      if (typeof picked === "string") setSource({ kind: chosenKind, path: picked });
      else if (Array.isArray(picked) && picked[0]) setSource({ kind: chosenKind, path: picked[0] });
    } catch (reason) { if (operationId === operation.current) setError(reason instanceof Error ? reason.message : "Could not open the source picker."); }
  };
  const scanSource = async () => {
    if (!source || !profileId) return;
    const operationId = ++operation.current;
    const requestedSource = source;
    const requestedProfileId = profileId;
    setBusy(true); setError(""); setStatus(""); setScan(null); setSelected(new Set());
    try {
      const result = await api.scanResourceImport({ profileId: requestedProfileId, source: requestedSource });
      if (operationId !== operation.current) return;
      setScan(result); setSelected(new Set(result.candidates.map((candidate) => candidate.candidateId)));
      setStatus(result.candidates.length ? `${result.candidates.length} candidates ready to review. Nothing has been imported.` : "No supported resources were found. The source was not changed.");
    } catch (reason) { if (operationId === operation.current) setError(reason instanceof Error ? reason.message : "Could not scan this source."); }
    finally { if (operationId === operation.current) setBusy(false); }
  };
  const commit = async () => {
    if (!scan || !selected.size) return;
    const operationId = ++operation.current;
    const requestedScan = scan;
    const candidateIds = [...selected];
    const requestedProfileId = profileId;
    setBusy(true); setError(""); setStatus("");
    try {
      const result = await api.commitResourceImport({ profileId: requestedProfileId, source: requestedScan.source, scanToken: requestedScan.scanToken, candidateIds, duplicatePolicy });
      if (operationId !== operation.current) return;
      const message = `Imported ${result.importedCount} resource${result.importedCount === 1 ? "" : "s"}; skipped ${result.skippedCount}.`;
      setScan(null); setSource(null); setSelected(new Set()); setStatus(message); onImported(message);
    } catch (reason) { if (operationId === operation.current) setError(reason instanceof Error ? reason.message : "Import failed. No partial batch was applied."); }
    finally { if (operationId === operation.current) setBusy(false); }
  };

  return <section className="resource-import-manager" aria-labelledby="resource-import-title">
    <div className="welcome-row"><div><h1 id="resource-import-title">Import resources</h1><p>Find links and items in a folder, Start Menu folder, or browser bookmark export.</p></div></div>
    <div className="resource-import-card">
      <h2>Choose a source</h2>
      <p className="resource-import-note">Scanning is read-only and limited to the selected source. Imported items are never opened automatically.</p>
      <div className="resource-import-controls">
        <label>Source type<select aria-label="Source type" value={choice} disabled={busy} onChange={(event) => { operation.current += 1; setBusy(false); setChoice(event.target.value as SourceChoice); setSource(null); setScan(null); setSelected(new Set()); setError(""); }}>
          <option value="directory">Directory</option><option value="start_menu">Start Menu folder</option><option value="bookmarks">Bookmark HTML file</option>
        </select></label>
        <button className="button secondary" onClick={() => void chooseSource()} disabled={busy}>Choose {sourceLabel[choice].toLowerCase()}</button>
      </div>
      {source && <div className="resource-import-source"><span>{sourceLabel[source.kind]} selected</span><code title={source.path}>{source.path}</code><button className="button primary" onClick={() => void scanSource()} disabled={busy}>{busy ? "Scanning…" : "Scan selected source"}</button></div>}
      <div className="resource-import-limits">Directory scans stop after depth 4 or 1,000 entries. Bookmark files are limited to 4 MiB. Symbolic links are skipped. Only HTTPS bookmarks and internet shortcuts are imported.</div>
      {error && <p className="form-error resource-import-error" role="alert">{error}</p>}
      {status && <p className="resource-import-status" role="status">{status}</p>}
    </div>
    {scan && <div className="resource-import-card resource-import-preview" aria-label="Import preview">
      <header><div><h2>Review candidates</h2><p>{scan.candidates.length} candidates · {existingCount} already in this profile · {scan.ignoredCount} unsupported or excluded</p></div></header>
      <fieldset className="resource-import-policy"><legend>When a target already exists in this profile</legend>
        <label><input type="radio" name="duplicate-policy" value="skip_existing" checked={duplicatePolicy === "skip_existing"} onChange={() => setDuplicatePolicy("skip_existing")} /> Skip existing targets</label>
        <label><input type="radio" name="duplicate-policy" value="create_duplicate" checked={duplicatePolicy === "create_duplicate"} onChange={() => setDuplicatePolicy("create_duplicate")} /> Create another resource</label>
      </fieldset>
      <div className="resource-import-table-wrap"><table className="resource-import-table"><thead><tr><th><input aria-label="Select all candidates" type="checkbox" checked={scan.candidates.length > 0 && selected.size === scan.candidates.length} onChange={(event) => setSelected(event.target.checked ? new Set(scan.candidates.map((candidate) => candidate.candidateId)) : new Set())} /></th><th>Resource</th><th>Target</th><th>Source</th><th>Match</th></tr></thead>
        <tbody>{scan.candidates.map((candidate) => <tr key={candidate.candidateId}>
          <td><input aria-label={`Select ${candidate.displayName}`} type="checkbox" checked={selected.has(candidate.candidateId)} onChange={(event) => setSelected((current) => { const next = new Set(current); event.target.checked ? next.add(candidate.candidateId) : next.delete(candidate.candidateId); return next; })} /></td>
          <td><b>{candidate.displayName}</b><small>{candidate.resourceType}</small></td><td><code title={candidate.targetUri}>{candidate.targetUri}</code></td><td><b>{candidate.sourceLabel}</b><small>{candidate.provenance}</small></td>
          <td>{candidate.existingResourceId ? <span className="resource-import-match">Already in profile</span> : <span>New</span>}</td>
        </tr>)}</tbody></table></div>
      <footer><span>{selected.size} selected</span><button className="button secondary" disabled={busy} onClick={() => { operation.current += 1; setScan(null); setSelected(new Set()); setStatus("Preview discarded. No resources were imported."); }}>Discard preview</button><button className="button primary" disabled={busy || selected.size === 0} onClick={() => void commit()}>{busy ? "Importing…" : `Import ${selected.size} selected`}</button></footer>
    </div>}
  </section>;
}
