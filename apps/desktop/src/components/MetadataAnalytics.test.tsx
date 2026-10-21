import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
const { api } = vi.hoisted(() => ({ api: { inspect: vi.fn(), analyticsSummary: vi.fn(), analyticsEvents: vi.fn(), isNativeApp: vi.fn() } }));
vi.mock("../lib/api", () => ({ api }));
import { ResourceMetadataPanel } from "./ResourceMetadataPanel";
import { AnalyticsView } from "./AnalyticsView";
import type { Resource } from "../lib/contracts";
const resource = (id: string): Resource => ({ id, resourceType: "file", displayName: id, defaultAction: "open", createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z", targets: [{ id: `t-${id}`, kind: "file", canonicalUri: `C:\\Work\\${id}`, priority: 0, arguments: [], active: true }], tags: [], aliases: [], favorite: false, launchCount: 0 });
let host: HTMLDivElement; let root: Root;
beforeEach(() => { host = document.createElement("div"); document.body.append(host); root = createRoot(host); api.isNativeApp.mockReturnValue(true); });
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
  it("shows native inspection failures", async () => {
    api.inspect.mockRejectedValue(new Error("Access denied"));
    await act(async () => root.render(createElement(ResourceMetadataPanel, { resource: resource("r1") })));
    expect(host.querySelector('[role="alert"]')?.textContent).toBe("Access denied");
  });
});

describe("AnalyticsView", () => {
  it("shows source/status/day counts from app-recorded launches", async () => {
    api.analyticsSummary.mockResolvedValue({ totalLaunches: 3, bySource: { dashboard: 2, dock: 1 }, byStatus: { handed_off: 2, failed: 1 }, firstEventUtc: "2026-01-01T00:00:00Z", lastEventUtc: "2026-01-02T00:00:00Z" });
    api.analyticsEvents.mockResolvedValue([
      { requestId: "1", resourceId: "r1", source: "dashboard", status: "handed_off", launchedAtUtc: "2026-01-01T10:00:00Z" },
      { requestId: "2", resourceId: "r1", source: "dock", status: "failed", launchedAtUtc: "2026-01-01T11:00:00Z" },
      { requestId: "3", resourceId: "r1", source: "dashboard", status: "handed_off", launchedAtUtc: "2026-01-02T11:00:00Z" },
    ]);
    await act(async () => root.render(createElement(AnalyticsView, { profileId: "p1" })));
    expect(host.textContent).toContain("App-recorded launch history"); expect(host.textContent).toContain("3"); expect(host.textContent).toContain("Dashboard"); expect(host.textContent).toContain("Handed Off"); expect(host.textContent).toContain("2026-01-01"); expect(host.textContent).toContain("does not observe files opened in other apps");
    expect(api.analyticsSummary).toHaveBeenCalledWith("p1"); expect(api.analyticsEvents).toHaveBeenCalledWith("p1", 500);
  });
});
