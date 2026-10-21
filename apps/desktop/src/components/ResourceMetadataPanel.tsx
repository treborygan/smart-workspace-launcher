import { useEffect, useState } from "react";
import type { Resource } from "../lib/contracts";
import type { ResourceMetadata } from "@sw/contracts";
import { api } from "../lib/api";

export function ResourceMetadataPanel({ resource }: { resource: Resource }) {
  const [records, setRecords] = useState<ResourceMetadata[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState("");
  const [version, setVersion] = useState(0);
  useEffect(() => {
    let current = true;
    setLoading(true); setError("");
    api.inspect(resource.id, version > 0).then((result) => { if (current) setRecords(result); })
      .catch((reason: unknown) => { if (current) setError(reason instanceof Error ? reason.message : "Metadata inspection failed."); })
      .finally(() => { if (current) { setLoading(false); setRefreshing(false); } });
    return () => { current = false; };
  }, [resource.id, version]);
  const refresh = () => { setRefreshing(true); setVersion((value) => value + 1); };
  const byTarget = new Map(records.map((item) => [item.targetId, item]));
  return <div className="metadata-panel" aria-live="polite">
    <div className="inspector-label metadata-heading">LOCAL TARGET INSPECTION <button className="subtle-action" disabled={loading || refreshing || !api.isNativeApp()} onClick={refresh}>{refreshing ? "Refreshing…" : "Refresh"}</button></div>
    {!api.isNativeApp() ? <p className="metadata-message">Metadata inspection is available in the Windows desktop app.</p> : loading ? <p className="metadata-message" role="status">Inspecting saved target…</p> : error ? <p className="metadata-error" role="alert">{error}</p> : records.length === 0 ? <p className="metadata-message">No inspectable target metadata is available.</p> : resource.targets.map((target) => {
      const item = byTarget.get(target.id);
      return <div className="metadata-target" key={target.id}><small className="metadata-path" title={target.canonicalUri}>{target.canonicalUri}</small>{item ? <>
        <Info label="Availability" value={label(item.availability)} />
        <Info label="Size" value={item.fileSizeBytes == null ? "Unavailable" : formatBytes(item.fileSizeBytes)} />
        <Info label="Created" value={formatDate(item.createdUtc)} />
        <Info label="Modified" value={formatDate(item.modifiedUtc)} />
        <Info label="Cache" value={label(item.cacheStatus)} />
        <Info label="Source" value={item.provenance === "local-filesystem" ? "Local file system" : "Unsupported target"} />
        {item.errorCode && <small className="metadata-error">Inspection status: {label(item.errorCode)}</small>}
        <small className="metadata-inspected">Inspected {formatDate(item.inspectedUtc)}</small>
      </> : <small className="metadata-message">No metadata returned for this target.</small>}</div>;
    })}
    <p className="provenance">Metadata comes from inspecting the saved target locally. Smart Workspace does not observe files opened in other apps.</p>
  </div>;
}
function Info({ label, value }: { label: string; value: string }) { return <div className="metadata-row"><span>{label}</span><b>{value}</b></div>; }
function label(value: string) { return value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase()); }
function formatDate(value?: string) { if (!value) return "Unavailable"; const date = new Date(value); return Number.isNaN(date.getTime()) ? value : date.toLocaleString(); }
function formatBytes(value: number) { if (value < 1024) return `${value} B`; const units = ["KB", "MB", "GB", "TB"]; let amount = value; let unit = "B"; for (const next of units) { amount /= 1024; unit = next; if (amount < 1024) break; } return `${amount.toFixed(amount < 10 ? 1 : 0)} ${unit}`; }
