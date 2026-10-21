import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Profile, Resource } from "./lib/contracts";

const { api } = vi.hoisted(() => ({
  api: { resources: vi.fn(), profiles: vi.fn(), create: vi.fn(), update: vi.fn(), remove: vi.fn(), launch: vi.fn() },
}));
vi.mock("./lib/api", () => ({ api }));
import App from "./App";

const timestamp = "2026-10-21T10:00:00.000Z";
const resources: Resource[] = [
  { id: "r1", resourceType: "file", displayName: "Roadmap.xlsx", description: "Planning workbook", defaultAction: "open", createdUtc: timestamp, updatedUtc: timestamp, targets: [{ id: "t1", kind: "file", canonicalUri: "C:\\Work\\Roadmap.xlsx", priority: 0, arguments: [], active: true }], tags: [], aliases: [], favorite: true, launchCount: 2, category: "Work" },
  { id: "r2", resourceType: "url", displayName: "Calendar", description: "Team calendar", defaultAction: "open", createdUtc: timestamp, updatedUtc: timestamp, targets: [{ id: "t2", kind: "url", canonicalUri: "https://calendar.example", priority: 0, arguments: [], active: true }], tags: [], aliases: [], favorite: false, launchCount: 4, category: "Web" },
];
const profiles: Profile[] = [{ id: "p1", name: "Personal", themeId: "system", settingsJson: "{}", createdUtc: timestamp, updatedUtc: timestamp }];
let root: Root;
let host: HTMLDivElement;

beforeEach(() => {
  localStorage.clear(); api.resources.mockResolvedValue(resources); api.profiles.mockResolvedValue(profiles);
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
});
afterEach(async () => { await act(async () => root.unmount()); host.remove(); vi.clearAllMocks(); });

describe("workspace dashboard", () => {
  it("loads the saved resources and exposes the inspector", async () => {
    await act(async () => { root.render(createElement(App)); });
    expect(host.textContent).toContain("Roadmap.xlsx");
    expect(host.textContent).toContain("Pinned resources");
    expect(host.textContent).toContain("Inspector");
  });

  it("opens Quick Launch from the keyboard and filters resources", async () => {
    await act(async () => { root.render(createElement(App)); });
    await act(async () => { window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 15)); });
    const input = host.querySelector<HTMLInputElement>("#global-search");
    expect(input).not.toBeNull();
    expect(document.activeElement).toBe(input);
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, "calendar"); input!.dispatchEvent(new Event("input", { bubbles: true })); });
    expect(host.textContent).toContain("Calendar");
    expect(host.querySelectorAll(".resource-card")).toHaveLength(1);
    expect(host.querySelector(".resource-card h3")?.textContent).toBe("Calendar");
  });
});
