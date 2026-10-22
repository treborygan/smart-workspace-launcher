import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DashboardWidget, ResourceRelationship, WidgetTemplate } from "@sw/contracts";
const { api } = vi.hoisted(() => ({ api: { analyticsEvents: vi.fn(), analyticsSummary: vi.fn(), usageSettings: vi.fn() } }));
vi.mock("../lib/api", () => ({ api }));
import { DashboardWidgets } from "./DashboardWidgets";
import type { Resource } from "../lib/contracts";

const resources: Resource[] = [
  { id: "r1", resourceType: "file", displayName: "Roadmap.xlsx", defaultAction: "open", createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z", targets: [{ id: "t1", kind: "file", canonicalUri: "C:\\Work\\Roadmap.xlsx", priority: 0, arguments: [], active: true }], tags: [], aliases: [], favorite: false, launchCount: 1 },
  { id: "r2", resourceType: "folder", displayName: "Projects", defaultAction: "open", createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z", targets: [{ id: "t2", kind: "folder", canonicalUri: "C:\\Work\\Projects", priority: 0, arguments: [], active: true }], tags: [], aliases: [], favorite: false, launchCount: 0 },
];
const template: WidgetTemplate = { id: "tpl1", name: "Pinned files", kind: "resource_shortcuts", config: { resourceIds: ["r1"] } };
const relationship: ResourceRelationship = { id: "edge1", profileId: "p1", sourceResourceId: "r1", targetResourceId: "r2", label: "project" };
let host: HTMLDivElement; let root: Root; let changed: ReturnType<typeof vi.fn<(templates: WidgetTemplate[], widgets: DashboardWidget[]) => void>>; let launched: ReturnType<typeof vi.fn<(resource: Resource) => void>>; let inspected: ReturnType<typeof vi.fn<(resourceId: string) => void>>; let notice: ReturnType<typeof vi.fn<(message: string) => void>>;
const render = (templates: WidgetTemplate[] = [], widgets: DashboardWidget[] = [], relationships: ResourceRelationship[] = []) => root.render(createElement(DashboardWidgets, { profileId: "p1", resources, relationships, templates, widgets, onChange: changed, onLaunch: launched, onInspect: inspected, onNotice: notice }));
beforeEach(() => { host = document.createElement("div"); document.body.append(host); root = createRoot(host); changed = vi.fn<(templates: WidgetTemplate[], widgets: DashboardWidget[]) => void>(); launched = vi.fn<(resource: Resource) => void>(); inspected = vi.fn<(resourceId: string) => void>(); notice = vi.fn<(message: string) => void>(); api.usageSettings.mockResolvedValue({ profileId: "p1", recordingEnabled: true, retentionDays: null, excludedResourceIds: [] }); api.analyticsEvents.mockResolvedValue([]); api.analyticsSummary.mockResolvedValue({ totalLaunches: 0, bySource: {}, byStatus: {} }); });
afterEach(async () => { await act(async () => root.unmount()); host.remove(); vi.resetAllMocks(); });

describe("DashboardWidgets", () => {
  it("creates a bounded resource shortcut template without launching anything", async () => {
    await act(async () => render());
    await act(async () => host.querySelector<HTMLButtonElement>("button[aria-expanded]")!.click());
    const name = host.querySelector<HTMLInputElement>('input[placeholder="e.g. Project starters"]')!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(name, "Launch files"); name.dispatchEvent(new Event("input", { bubbles: true })); });
    const checkbox = host.querySelector<HTMLInputElement>(".widget-resource-picker input")!;
    await act(async () => { checkbox.click(); });
    await act(async () => host.querySelector<HTMLButtonElement>(".widget-template-form .button.primary")!.click());
    expect(changed).toHaveBeenCalledTimes(1);
    const [saved] = changed.mock.calls[0] as [WidgetTemplate[], DashboardWidget[]];
    expect(saved[0]).toMatchObject({ name: "Launch files", kind: "resource_shortcuts", config: { resourceIds: ["r1"] } });
    expect(launched).not.toHaveBeenCalled();
  });

  it("adds and safely opens an instance from an existing template", async () => {
    await act(async () => render([template]));
    const add = [...host.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent === "Add widget")!;
    await act(async () => add.click());
    const [savedTemplates, savedWidgets] = changed.mock.calls.at(-1) as [WidgetTemplate[], DashboardWidget[]];
    expect(savedTemplates).toEqual([template]);
    expect(savedWidgets).toHaveLength(1);
    await act(async () => render([template], savedWidgets));
    expect(host.textContent).toContain("Roadmap.xlsx");
    expect(launched).not.toHaveBeenCalled();
    const open = [...host.querySelectorAll<HTMLButtonElement>(".widget-resource")].find((button) => button.textContent?.includes("Roadmap.xlsx"))!;
    await act(async () => open.click());
    expect(launched).toHaveBeenCalledWith(resources[0]);
  });

  it("renders related resources without changing the underlying graph", async () => {
    const related: WidgetTemplate = { id: "tpl2", name: "Project links", kind: "related_resources", config: { resourceId: "r1", limit: 3 } };
    await act(async () => render([related], [{ id: "w2", templateId: "tpl2", x: 0, y: 0, w: 4, h: 2 }], [relationship]));
    expect(host.textContent).toContain("Projects");
    expect(host.textContent).toContain("Related to");
    expect(launched).not.toHaveBeenCalled();
    const anchor = [...host.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent === "Roadmap.xlsx")!;
    await act(async () => anchor.click());
    expect(inspected).toHaveBeenCalledWith("r1");
  });

  it("shows the local-history provenance and stops stale profile responses", async () => {
    const recent: WidgetTemplate = { id: "tpl3", name: "Recent", kind: "recent_launches", config: { limit: 3 } };
    api.usageSettings.mockResolvedValue({ profileId: "p1", recordingEnabled: false, retentionDays: 30, excludedResourceIds: [] });
    api.analyticsEvents.mockResolvedValue([{ requestId: "req1", resourceId: "r1", source: "dashboard", status: "handed_off", launchedAtUtc: "2026-01-01T00:00:00Z" }]);
    api.analyticsSummary.mockResolvedValue({ totalLaunches: 1, bySource: { dashboard: 1 }, byStatus: { handed_off: 1 } });
    await act(async () => render([recent], [{ id: "w3", templateId: "tpl3", x: 0, y: 0, w: 4, h: 2 }]));
    await act(async () => { await Promise.resolve(); await Promise.resolve(); });
    expect(host.textContent).toContain("Recording is off. Previously retained events are shown.");
    expect(host.textContent).toContain("Roadmap.xlsx");
    expect(api.analyticsEvents).toHaveBeenCalledWith({ profileId: "p1", limit: 3, offset: 0 });
  });

  it("removes a widget instance without deleting its reusable template", async () => {
    await act(async () => render([template], [{ id: "w1", templateId: "tpl1", x: 0, y: 0, w: 4, h: 2 }]));
    const remove = [...host.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.getAttribute("aria-label") === "Remove Pinned files widget")!;
    await act(async () => remove.click());
    expect(changed).toHaveBeenCalledWith([template], []);
  });
});
