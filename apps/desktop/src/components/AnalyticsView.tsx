import { useEffect, useMemo, useState } from "react";
import type { AnalyticsSummary, LaunchEvent, LaunchSource, LaunchStatus, Resource, UsageFilter, UsageSettings } from "@sw/contracts";
import { api } from "../lib/api";
import { chooseUsageExportPath } from "../lib/usageDialogs";

const PAGE_SIZE = 50;
const EMPTY_SUMMARY: AnalyticsSummary = { totalLaunches: 0, bySource: {}, byStatus: {} };
const EMPTY_FILTER: UsageFilter = {};

export function AnalyticsView({ profileId }: { profileId?: string }) {
  const [settings, setSettings] = useState<UsageSettings | null>(null);
  const [settingsDraft, setSettingsDraft] = useState<UsageSettings | null>(null);
  const [resourceState, setResourceState] = useState<{ profileId: string; resources: Resource[] } | null>(null);
  const [resultProfileId, setResultProfileId] = useState<string | null>(null);
  const [summary, setSummary] = useState<AnalyticsSummary>(EMPTY_SUMMARY);
  const [events, setEvents] = useState<LaunchEvent[]>([]);
  const [pageState, setPageState] = useState<{ profileId: string; filters: UsageFilter; offset: number }>({ profileId: profileId || "", filters: EMPTY_FILTER, offset: 0 });
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [reload, setReload] = useState(0);
  const filters = pageState.profileId === profileId ? pageState.filters : EMPTY_FILTER;
  const offset = pageState.profileId === profileId ? pageState.offset : 0;
  const resources = resourceState && resourceState.profileId === profileId ? resourceState.resources : [];
  const currentSettings = settings?.profileId === profileId ? settings : null;
  const currentSettingsDraft = settingsDraft?.profileId === profileId ? settingsDraft : null;
  const hasCurrentResults = resultProfileId === profileId;
  const visibleSummary = hasCurrentResults ? summary : EMPTY_SUMMARY;
  const visibleEvents = hasCurrentResults ? events : [];
  const query = useMemo(() => ({ profileId: profileId || "", ...filters, limit: PAGE_SIZE, offset }), [profileId, filters, offset]);

  useEffect(() => {
    setPageState({ profileId: profileId || "", filters: EMPTY_FILTER, offset: 0 });
  }, [profileId]);

  useEffect(() => {
    if (!profileId) return;
    let current = true;
    setError("");
    Promise.all([api.usageSettings(profileId), api.resources(profileId)])
      .then(([nextSettings, nextResources]) => {
        if (!current) return;
        setSettings(nextSettings); setSettingsDraft(nextSettings); setResourceState({ profileId, resources: nextResources });
      })
      .catch((reason: unknown) => { if (current) setError(errorText(reason, "Usage settings could not be loaded.")); });
    return () => { current = false; };
  }, [profileId, reload]);

  useEffect(() => {
    if (!profileId) return;
    let current = true;
    setLoading(true); setError("");
    Promise.all([api.analyticsSummary(profileId, filters), api.analyticsEvents(query)])
      .then(([nextSummary, nextEvents]) => { if (current) { setSummary(nextSummary); setEvents(nextEvents); setResultProfileId(profileId); } })
      .catch((reason: unknown) => { if (current) setError(errorText(reason, "Launch history could not be loaded.")); })
      .finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, [profileId, filters, query, reload]);

  const setFilter = <K extends keyof UsageFilter>(key: K, value: UsageFilter[K] | undefined) => {
    if (!profileId) return;
    setPageState((current) => ({ profileId, filters: { ...(current.profileId === profileId ? current.filters : EMPTY_FILTER), [key]: value || undefined }, offset: 0 }));
  };
  const updateSettings = <K extends keyof UsageSettings>(key: K, value: UsageSettings[K]) => {
    setSettingsDraft((current) => current ? { ...current, [key]: value } : current);
  };
  const saveSettings = async () => {
    if (!profileId || settingsDraft?.profileId !== profileId) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const saved = await api.saveUsageSettings(settingsDraft);
      setSettings(saved); setSettingsDraft(saved); setNotice("Usage settings saved."); setReload((value) => value + 1);
    } catch (reason) { setError(errorText(reason, "Usage settings could not be saved.")); }
    finally { setBusy(false); }
  };
  const purgeHistory = async () => {
    if (!profileId || !window.confirm("Permanently delete this profile’s recorded launch history? This does not affect your resources or replay protection.")) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const deleted = await api.purgeUsageHistory(profileId);
      setNotice(`${deleted} recorded event${deleted === 1 ? "" : "s"} deleted.`); setPageState({ profileId, filters, offset: 0 }); setReload((value) => value + 1);
    } catch (reason) { setError(errorText(reason, "Recorded launch history could not be deleted.")); }
    finally { setBusy(false); }
  };
  const exportCsv = async () => {
    if (!profileId) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const path = await chooseUsageExportPath();
      if (!path) return;
      const result = await api.exportUsageCsv({ profileId, ...filters }, path);
      setNotice(`Exported ${result.eventCount} recorded event${result.eventCount === 1 ? "" : "s"}.`);
    } catch (reason) { setError(errorText(reason, "Usage CSV could not be exported.")); }
    finally { setBusy(false); }
  };

  if (!profileId) return <section className="analytics-view"><div className="metadata-error" role="alert">Select a workspace profile to view its usage.</div></section>;
  const isDirty = currentSettingsDraft && JSON.stringify(currentSettingsDraft) !== JSON.stringify(currentSettings);
  const resourceNames = new Map(resources.map((resource) => [resource.id, resource.displayName]));
  const canGoBack = offset > 0;
  const canGoNext = hasCurrentResults && offset + visibleEvents.length < visibleSummary.totalLaunches;

  return <section className="analytics-view">
    <div className="welcome-row"><div><h1>Activity</h1><p>Recorded launch requests for this workspace profile.</p></div><div className="analytics-actions"><button className="button secondary" onClick={() => setReload((value) => value + 1)} disabled={loading || busy}>Refresh</button><button className="button secondary" onClick={() => void exportCsv()} disabled={busy || !api.isNativeApp()}>Export CSV</button></div></div>
    <p className="analytics-note analytics-scope">This local history includes requests recorded by Smart Workspace on this device. A successful handoff means Windows received the request; it does not confirm the target opened. The app does not observe files opened in other apps.</p>
    {!api.isNativeApp() && <p className="metadata-message">The browser preview does not contain native launch history or CSV export.</p>}
    {error && <div className="metadata-error" role="alert">{error}</div>}{notice && <div className="metadata-message" role="status">{notice}</div>}

    {currentSettingsDraft && <section className="analytics-card usage-settings" aria-labelledby="usage-settings-title">
      <div className="analytics-settings-heading"><div><h2 id="usage-settings-title">History settings</h2><p>Controls affect this profile on this device.</p></div><button className="button primary" onClick={() => void saveSettings()} disabled={busy || !isDirty}>{busy ? "Saving…" : "Save settings"}</button></div>
      <label className="usage-recording"><input type="checkbox" checked={currentSettingsDraft.recordingEnabled} onChange={(event) => updateSettings("recordingEnabled", event.target.checked)} /><span><b>Record new launch requests</b><small>Turning this off keeps existing history. Use Delete history to remove it.</small></span></label>
      <label className="usage-retention">Keep recorded events<select value={currentSettingsDraft.retentionDays ?? "forever"} onChange={(event) => updateSettings("retentionDays", event.target.value === "forever" ? null : Number(event.target.value) as 30 | 90 | 365)}><option value="forever">Until I delete them</option><option value="30">30 days</option><option value="90">90 days</option><option value="365">365 days</option></select></label>
      <details className="usage-exclusions"><summary>Exclude resources from history ({currentSettingsDraft.excludedResourceIds.length})</summary>
        <div className="usage-exclusion-list">{resources.map((resource) => <label key={resource.id}><input type="checkbox" checked={currentSettingsDraft.excludedResourceIds.includes(resource.id)} onChange={(event) => updateSettings("excludedResourceIds", event.target.checked ? [...currentSettingsDraft.excludedResourceIds, resource.id] : currentSettingsDraft.excludedResourceIds.filter((id) => id !== resource.id))} /><span>{resource.displayName}</span></label>)}{resources.length === 0 && <p>No resources in this profile.</p>}</div>
      </details>
      <button className="button danger" onClick={() => void purgeHistory()} disabled={busy}>Delete recorded history…</button>
    </section>}

    <div className="analytics-filters" aria-label="Filter usage history">
      <label>From (UTC)<input type="date" value={dateInput(filters.fromUtc)} onChange={(event) => setFilter("fromUtc", event.target.value ? `${event.target.value}T00:00:00.000Z` : undefined)} /></label>
      <label>To (UTC)<input type="date" value={dateInput(filters.toUtc)} onChange={(event) => setFilter("toUtc", event.target.value ? `${event.target.value}T23:59:59.999Z` : undefined)} /></label>
      <label>Source<select value={filters.source || ""} onChange={(event) => setFilter("source", (event.target.value || undefined) as LaunchSource | undefined)}><option value="">All sources</option>{["dashboard", "quick_launch", "dock", "explorer", "sequence", "other"].map((value) => <option key={value} value={value}>{title(value)}</option>)}</select></label>
      <label>Status<select value={filters.status || ""} onChange={(event) => setFilter("status", (event.target.value || undefined) as LaunchStatus | undefined)}><option value="">All statuses</option>{["requested", "handed_off", "failed"].map((value) => <option key={value} value={value}>{title(value)}</option>)}</select></label>
      <label>Resource<select value={filters.resourceId || ""} onChange={(event) => setFilter("resourceId", event.target.value || undefined)}><option value="">All resources</option>{resources.map((resource) => <option key={resource.id} value={resource.id}>{resource.displayName}</option>)}</select></label>
      <button className="button subtle" onClick={() => { if (profileId) setPageState({ profileId, filters: EMPTY_FILTER, offset: 0 }); }}>Clear filters</button>
    </div>
    <p className="analytics-note">Filters reset when you switch workspace profiles.</p>

    <div className="analytics-summary"><div><span>Matching recorded requests</span><b>{visibleSummary.totalLaunches}</b></div><div><span>First matching event</span><b className="analytics-date">{formatDate(visibleSummary.firstEventUtc)}</b></div><div><span>Most recent matching event</span><b className="analytics-date">{formatDate(visibleSummary.lastEventUtc)}</b></div></div>
    <div className="analytics-columns"><section className="analytics-card"><h2>By launch source</h2><Breakdown values={visibleSummary.bySource} empty="No source data for these filters." /></section><section className="analytics-card"><h2>By recorded status</h2><Breakdown values={visibleSummary.byStatus} empty="No status data for these filters." /></section></div>
    <div className="section-heading"><div><h2>Recorded requests</h2><span>{visibleSummary.totalLaunches} matching event{visibleSummary.totalLaunches === 1 ? "" : "s"}</span></div></div>
    {loading ? <div className="loading-state" role="status">Loading recorded events…</div> : visibleEvents.length ? <div className="table-shell"><table><thead><tr><th>WHEN (UTC)</th><th>RESOURCE</th><th>SOURCE</th><th>STATUS</th><th>DETAIL</th></tr></thead><tbody>{visibleEvents.map((event) => <tr key={event.requestId}><td>{formatUtc(event.launchedAtUtc)}</td><td>{resourceNames.get(event.resourceId) || "Removed resource"}</td><td>{title(event.source)}</td><td>{title(event.status)}</td><td>{event.errorCode ? title(event.errorCode) : event.status === "handed_off" ? "Request handed to Windows" : "—"}</td></tr>)}</tbody></table></div> : <div className="table-empty">{visibleSummary.totalLaunches ? "No events on this page." : "No recorded launch requests match these filters."}</div>}
    <div className="usage-pagination"><span>{visibleSummary.totalLaunches ? `${offset + 1}–${Math.min(offset + visibleEvents.length, visibleSummary.totalLaunches)} of ${visibleSummary.totalLaunches}` : "0 events"}</span><div><button className="button secondary" disabled={!canGoBack || loading} onClick={() => profileId && setPageState({ profileId, filters, offset: Math.max(0, offset - PAGE_SIZE) })}>Previous</button><button className="button secondary" disabled={!canGoNext || loading} onClick={() => profileId && setPageState({ profileId, filters, offset: offset + PAGE_SIZE })}>Next</button></div></div>
  </section>;
}

function Breakdown({ values, empty }: { values: Record<string, number>; empty: string }) { const entries = Object.entries(values).sort((a, b) => b[1] - a[1]); return entries.length ? <div>{entries.map(([key, count]) => <div className="analytics-breakdown" key={key}><span>{title(key)}</span><b>{count}</b></div>)}</div> : <p className="metadata-message">{empty}</p>; }
function title(value: string) { return value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase()); }
function dateInput(value?: string) { return value ? value.slice(0, 10) : ""; }
function formatDate(value?: string) { if (!value) return "—"; const date = new Date(value); return Number.isNaN(date.getTime()) ? value : date.toLocaleDateString(); }
function formatUtc(value: string) { const date = new Date(value); return Number.isNaN(date.getTime()) ? value : date.toISOString().replace("T", " ").replace(".000Z", " UTC").replace(/\.\d{3}Z$/, " UTC"); }
function errorText(reason: unknown, fallback: string) { return reason instanceof Error ? reason.message : fallback; }
