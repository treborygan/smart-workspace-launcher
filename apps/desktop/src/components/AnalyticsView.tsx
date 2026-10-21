import { useEffect, useState } from "react";
import type { AnalyticsSummary, LaunchEvent } from "@sw/contracts";
import { api } from "../lib/api";

export function AnalyticsView({ profileId }: { profileId?: string }) {
  const [summary, setSummary] = useState<AnalyticsSummary | null>(null);
  const [events, setEvents] = useState<LaunchEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [reload, setReload] = useState(0);
  useEffect(() => {
    let current = true;
    setLoading(true); setError("");
    Promise.all([api.analyticsSummary(profileId), api.analyticsEvents(profileId, 500)])
      .then(([nextSummary, nextEvents]) => { if (current) { setSummary(nextSummary); setEvents(nextEvents); } })
      .catch((reason: unknown) => { if (current) setError(reason instanceof Error ? reason.message : "Launch history could not be loaded."); })
      .finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, [profileId, reload]);
  const perDay = events.reduce<Record<string, number>>((days, event) => { const day = event.launchedAtUtc.slice(0, 10); days[day] = (days[day] || 0) + 1; return days; }, {});
  return <>
    <div className="welcome-row"><div><h1>Activity</h1><p>App-recorded launch history for this workspace profile.</p></div><button className="button secondary" onClick={() => setReload((value) => value + 1)} disabled={loading}>Refresh</button></div>
    <p className="analytics-note analytics-scope">These events are launch requests recorded by Smart Workspace on this device. The app does not observe files opened in other apps.</p>
    {!api.isNativeApp() ? <div className="table-empty">Launch history is available in the Windows desktop app.</div> : loading ? <div className="loading-state" role="status">Loading recorded launch events…</div> : error ? <div className="metadata-error" role="alert">{error}</div> : <>
      <div className="analytics-summary"><div><span>Recorded launches</span><b>{summary?.totalLaunches ?? events.length}</b></div><div><span>First event</span><b className="analytics-date">{formatDate(summary?.firstEventUtc)}</b></div><div><span>Most recent</span><b className="analytics-date">{formatDate(summary?.lastEventUtc)}</b></div></div>
      <div className="analytics-columns"><section className="analytics-card"><h2>By launch source</h2><Breakdown values={summary?.bySource || {}} empty="No source data recorded." /></section><section className="analytics-card"><h2>By recorded status</h2><Breakdown values={summary?.byStatus || {}} empty="No status data recorded." /></section></div>
      <div className="section-heading"><div><h2>Activity by day</h2><span>{events.length} recent events</span></div></div>
      {Object.keys(perDay).length ? <div className="analytics-days">{Object.entries(perDay).sort(([a], [b]) => b.localeCompare(a)).map(([day, count]) => <div className="analytics-day" key={day}><span>{day}</span><div className="analytics-bar-track"><i style={{ width: `${Math.max(5, count / Math.max(...Object.values(perDay)) * 100)}%` }} /></div><b>{count}</b></div>)}</div> : <div className="table-empty">No launch events have been recorded for this profile yet.</div>}
    </>}
  </>;
}
function Breakdown({ values, empty }: { values: Record<string, number>; empty: string }) { const entries = Object.entries(values).sort((a, b) => b[1] - a[1]); return entries.length ? <div>{entries.map(([key, count]) => <div className="analytics-breakdown" key={key}><span>{title(key)}</span><b>{count}</b></div>)}</div> : <p className="metadata-message">{empty}</p>; }
function title(value: string) { return value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase()); }
function formatDate(value?: string) { if (!value) return "—"; const date = new Date(value); return Number.isNaN(date.getTime()) ? value : date.toLocaleDateString(); }
