import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
const { api, chooseUsageExportPath } = vi.hoisted(() => ({ api: { inspect: vi.fn(), analyticsSummary: vi.fn(), analyticsEvents: vi.fn(), usageSettings: vi.fn(), saveUsageSettings: vi.fn(), resources: vi.fn(), isNativeApp: vi.fn(), purgeUsageHistory: vi.fn(), exportUsageCsv: vi.fn() }, chooseUsageExportPath: vi.fn() }));
vi.mock("../lib/api", () => ({ api }));
vi.mock("../lib/usageDialogs", () => ({ chooseUsageExportPath }));
import { ResourceMetadataPanel } from "./ResourceMetadataPanel";
import { AnalyticsView } from "./AnalyticsView";
import type { Resource } from "../lib/contracts";
const resource = (id: string): Resource => ({ id, resourceType: "file", displayName: id, defaultAction: "open", createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z", targets: [{ id: `t-${id}`, kind: "file", canonicalUri: `C:\\Work\\${id}`, priority: 0, arguments: [], active: true }], tags: [], aliases: [], favorite: false, launchCount: 0 });
let host: HTMLDivElement; let root: Root;
beforeEach(() => { host = document.createElement("div"); document.body.append(host); root = createRoot(host); api.isNativeApp.mockReturnValue(true); api.usageSettings.mockResolvedValue({ profileId: "p1", recordingEnabled: true, retentionDays: null, excludedResourceIds: [] }); api.resources.mockResolvedValue([resource("r1")]); });
afterEach(async () => { await act(async () => root.unmount()); host.remove(); vi.resetAllMocks(); });

describe("ResourceMetadataPanel", () => {
  it("renders local cached metadata and explicitly states its provenance", async () => {
    api.inspect.mockResolvedValue([{ resourceId: "r1", targetId: "t-r1", inspectedUtc: "2026-01-02T00:00:00Z", fileSizeBytes: 2048, createdUtc: "2026-01-01T00:00:00Z", modifiedUtc: "2026-01-02T00:00:00Z", availability: "available", provenance: "local-filesystem", cacheStatus: "cached" }]);
    await act(async () => root.render(createElement(ResourceMetadataPanel, { resource: resource("r1") })));
    expect(host.textContent).toContain("2.0 KB"); expect(host.textContent).toContain("Cached"); expect(host.textContent).toContain("Local file system");
    expect(host.textContent).toContain("does not observe files opened in other apps");
    expect(api.inspect).toHaveBeenCalledWith("r1", false);
  });
  it("ignores an inspection response after the selected resource changes", async () => {
    let resolveFirst!: (value: never[]) => void;
    api.inspect.mockImplementationOnce(() => new Promise((resolve) => { resolveFirst = resolve; })).mockResolvedValueOnce([]);
    await act(async () => root.render(createElement(ResourceMetadataPanel, { resource: resource("r1") })));
    await act(async () => root.render(createElement(ResourceMetadataPanel, { resource: resource("r2") })));
    await act(async () => { resolveFirst([{ resourceId: "r1", targetId: "t-r1", availability: "available" } as never]); });
    expect(api.inspect).toHaveBeenNthCalledWith(2, "r2", false);
    expect(host.textContent).not.toContain("C:\\Work\\r1");
  });
  it("re-inspects when a target path changes without a resource ID change", async () => {
    const first = resource("r1");
    const changed = { ...first, targets: [{ ...first.targets[0], canonicalUri: "C:\\Work\\renamed.xlsx" }] };
    api.inspect.mockResolvedValueOnce([{ resourceId: "r1", targetId: "t-r1", inspectedUtc: "2026-01-01T00:00:00Z", availability: "available", provenance: "local-filesystem", cacheStatus: "cached" }]);
    api.inspect.mockResolvedValueOnce([{ resourceId: "r1", targetId: "t-r1", inspectedUtc: "2026-01-02T00:00:00Z", availability: "missing", provenance: "local-filesystem", cacheStatus: "fresh" }]);
    await act(async () => root.render(createElement(ResourceMetadataPanel, { resource: first })));
    expect(host.textContent).toContain("Available");
    await act(async () => root.render(createElement(ResourceMetadataPanel, { resource: changed })));
    expect(api.inspect).toHaveBeenNthCalledWith(2, "r1", false);
    expect(host.textContent).toContain("C:\\Work\\renamed.xlsx");
    expect(host.textContent).toContain("Missing");
  });
  it("re-inspects when target kind changes with the same target ID and URI", async () => {
    const first = resource("r1");
    const changed = { ...first, targets: [{ ...first.targets[0], kind: "app" as const }] };
    api.inspect.mockResolvedValueOnce([{ resourceId: "r1", targetId: "t-r1", inspectedUtc: "2026-01-01T00:00:00Z", availability: "available", provenance: "local-filesystem", cacheStatus: "cached" }]);
    api.inspect.mockResolvedValueOnce([{ resourceId: "r1", targetId: "t-r1", inspectedUtc: "2026-01-02T00:00:00Z", availability: "unsupported", provenance: "unsupported", cacheStatus: "fresh" }]);
    await act(async () => root.render(createElement(ResourceMetadataPanel, { resource: first })));
    expect(host.textContent).toContain("Available");
    await act(async () => root.render(createElement(ResourceMetadataPanel, { resource: changed })));
    expect(api.inspect).toHaveBeenNthCalledWith(2, "r1", false);
    expect(host.textContent).toContain("Unsupported");
  });
  it("shows native inspection failures", async () => {
    api.inspect.mockRejectedValue(new Error("Access denied"));
    await act(async () => root.render(createElement(ResourceMetadataPanel, { resource: resource("r1") })));
    expect(host.querySelector('[role="alert"]')?.textContent).toBe("Access denied");
  });
});

describe("AnalyticsView", () => {
  it("shows filtered summary and paged event data with local provenance language", async () => {
    api.analyticsSummary.mockResolvedValue({ totalLaunches: 3, bySource: { dashboard: 2, dock: 1 }, byStatus: { handed_off: 2, failed: 1 }, firstEventUtc: "2026-01-01T00:00:00Z", lastEventUtc: "2026-01-02T00:00:00Z" });
    api.analyticsEvents.mockResolvedValue([
      { requestId: "1", resourceId: "r1", source: "dashboard", status: "handed_off", launchedAtUtc: "2026-01-01T10:00:00Z" },
      { requestId: "2", resourceId: "r1", source: "dock", status: "failed", launchedAtUtc: "2026-01-01T11:00:00Z" },
      { requestId: "3", resourceId: "r1", source: "dashboard", status: "handed_off", launchedAtUtc: "2026-01-02T11:00:00Z" },
    ]);
    await act(async () => root.render(createElement(AnalyticsView, { profileId: "p1" })));
    expect(host.textContent).toContain("Recorded launch requests"); expect(host.textContent).toContain("3"); expect(host.textContent).toContain("Dashboard"); expect(host.textContent).toContain("Handed Off"); expect(host.textContent).toContain("2026-01-01"); expect(host.textContent).toContain("does not observe files opened in other apps"); expect(host.textContent).toContain("does not confirm the target opened");
    expect(api.analyticsSummary).toHaveBeenCalledWith("p1", {}); expect(api.analyticsEvents).toHaveBeenCalledWith({ profileId: "p1", limit: 50, offset: 0 });
  });

  it("keeps recording disabled as a preference and requires explicit confirmation to purge", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    api.analyticsSummary.mockResolvedValue({ totalLaunches: 0, bySource: {}, byStatus: {} }); api.analyticsEvents.mockResolvedValue([]);
    await act(async () => root.render(createElement(AnalyticsView, { profileId: "p1" })));
    const recording = host.querySelector<HTMLInputElement>('input[type="checkbox"]')!;
    await act(async () => { recording.click(); });
    expect(host.textContent).toContain("Turning this off keeps existing history");
    api.saveUsageSettings.mockImplementation(async (settings) => settings);
    const saveButton = Array.from(host.querySelectorAll("button")).find((button) => button.textContent === "Save settings")!;
    await act(async () => { saveButton.click(); await Promise.resolve(); await Promise.resolve(); });
    expect(api.saveUsageSettings).toHaveBeenCalledWith({ profileId: "p1", recordingEnabled: false, retentionDays: null, excludedResourceIds: [] });
    const deleteButton = Array.from(host.querySelectorAll("button")).find((button) => button.textContent?.includes("Delete recorded history"))!;
    await act(async () => { deleteButton.click(); });
    expect(api.purgeUsageHistory).not.toHaveBeenCalled(); expect(confirm).toHaveBeenCalledTimes(1);
  });

  it("applies UTC date filters to both the summary and event page", async () => {
    api.analyticsSummary.mockResolvedValue({ totalLaunches: 0, bySource: {}, byStatus: {} }); api.analyticsEvents.mockResolvedValue([]);
    await act(async () => root.render(createElement(AnalyticsView, { profileId: "p1" })));
    const from = host.querySelector<HTMLInputElement>('input[type="date"]')!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(from, "2026-02-03"); from.dispatchEvent(new Event("input", { bubbles: true })); from.dispatchEvent(new Event("change", { bubbles: true })); });
    expect(api.analyticsSummary).toHaveBeenLastCalledWith("p1", { fromUtc: "2026-02-03T00:00:00.000Z" });
    expect(api.analyticsEvents).toHaveBeenLastCalledWith({ profileId: "p1", fromUtc: "2026-02-03T00:00:00.000Z", limit: 50, offset: 0 });
  });

  it("exports the active filters through a chosen CSV destination", async () => {
    api.analyticsSummary.mockResolvedValue({ totalLaunches: 1, bySource: {}, byStatus: {} }); api.analyticsEvents.mockResolvedValue([{ requestId: "1", resourceId: "r1", source: "dock", status: "handed_off", launchedAtUtc: "2026-01-01T10:00:00Z" }]);
    chooseUsageExportPath.mockResolvedValue("C:\\Exports\\usage.csv"); api.exportUsageCsv.mockResolvedValue({ path: "C:\\Exports\\usage.csv", eventCount: 1 });
    await act(async () => root.render(createElement(AnalyticsView, { profileId: "p1" })));
    const exportButton = Array.from(host.querySelectorAll("button")).find((button) => button.textContent === "Export CSV")!;
    await act(async () => { exportButton.click(); await Promise.resolve(); await Promise.resolve(); });
    expect(api.exportUsageCsv).toHaveBeenCalledWith({ profileId: "p1" }, "C:\\Exports\\usage.csv");
    expect(host.textContent).toContain("Exported 1 recorded event");
  });
});
