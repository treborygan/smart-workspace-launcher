import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Profile, Resource } from "./lib/contracts";

const { api } = vi.hoisted(() => ({
  api: { resources: vi.fn(), profiles: vi.fn(), layout: vi.fn(), saveLayout: vi.fn(), search: vi.fn(), onQuickLaunchToggle: vi.fn(), onQuickLaunchHotkeyStatus: vi.fn(), quickLaunchHotkeyStatus: vi.fn(), hideCurrentWindow: vi.fn(), create: vi.fn(), update: vi.fn(), remove: vi.fn(), launch: vi.fn() },
}));
vi.mock("./lib/api", () => ({ api }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ label: "quick-launch", onFocusChanged: async () => () => undefined }) }));
import App from "./App";

const timestamp = "2026-10-21T10:00:00.000Z";
const resources: Resource[] = [
  { id: "r1", resourceType: "file", displayName: "Roadmap.xlsx", description: "Planning workbook", defaultAction: "open", createdUtc: timestamp, updatedUtc: timestamp, targets: [{ id: "t1", kind: "file", canonicalUri: "C:\\Work\\Roadmap.xlsx", priority: 0, arguments: [], active: true }], tags: [], aliases: [], favorite: true, launchCount: 2, category: "Work" },
  { id: "r2", resourceType: "url", displayName: "Calendar", description: "Team calendar", defaultAction: "open", createdUtc: timestamp, updatedUtc: timestamp, targets: [{ id: "t2", kind: "url", canonicalUri: "https://calendar.example", priority: 0, arguments: [], active: true }], tags: [], aliases: [], favorite: false, launchCount: 4, category: "Web" },
];
const profiles: Profile[] = [
  { id: "p1", name: "Personal", themeId: "system", settingsJson: "{}", createdUtc: timestamp, updatedUtc: timestamp },
  { id: "p2", name: "Work", themeId: "system", settingsJson: "{}", createdUtc: timestamp, updatedUtc: timestamp },
];
let root: Root;
let host: HTMLDivElement;

beforeEach(() => {
  localStorage.clear(); api.resources.mockResolvedValue(resources); api.profiles.mockResolvedValue(profiles); api.layout.mockResolvedValue(null); api.saveLayout.mockImplementation(async (layout) => layout);
  api.search.mockImplementation(async (query: string, _profileId: string, fallback: Resource[]) => query ? fallback.filter((resource) => `${resource.displayName} ${resource.description}`.toLowerCase().includes(query.toLowerCase())) : fallback);
  api.onQuickLaunchToggle.mockResolvedValue(() => undefined); api.onQuickLaunchHotkeyStatus.mockResolvedValue(() => undefined);
  api.quickLaunchHotkeyStatus.mockResolvedValue({ registered: true, shortcut: "Alt+Space" });
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
});
afterEach(async () => { await act(async () => root.unmount()); host.remove(); Reflect.deleteProperty(window, "__TAURI_INTERNALS__"); vi.clearAllMocks(); });

describe("workspace dashboard", () => {
  it("loads the saved resources and exposes the inspector", async () => {
    await act(async () => { root.render(createElement(App)); });
    expect(host.textContent).toContain("Roadmap.xlsx");
    expect(host.textContent).toContain("Pinned resources");
    expect(host.textContent).toContain("Inspector");
  });

  it("opens Quick Launch from Ctrl+K and filters resources", async () => {
    await act(async () => { root.render(createElement(App)); });
    await act(async () => { window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 15)); });
    const input = host.querySelector<HTMLInputElement>(".quick-launch-search input");
    expect(input).not.toBeNull();
    expect(document.activeElement).toBe(input);
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, "calendar"); input!.dispatchEvent(new Event("input", { bubbles: true })); });
    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 100)); });
    expect(host.textContent).toContain("Calendar");
    expect(host.querySelectorAll(".quick-launch-result")).toHaveLength(1);
    expect(host.querySelector(".quick-launch-result b")?.textContent).toBe("Calendar");
  });

  it("uses Alt+Space as a browser fallback and launches the keyboard selection", async () => {
    api.launch.mockResolvedValue({ requestId: "launch-1", status: "handed_off", launchedAtUtc: timestamp });
    await act(async () => { root.render(createElement(App)); });
    await act(async () => { window.dispatchEvent(new KeyboardEvent("keydown", { code: "Space", key: " ", altKey: true, bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 20)); });
    const input = host.querySelector<HTMLInputElement>(".quick-launch-search input")!;
    expect(document.activeElement).toBe(input);
    await act(async () => { input.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })); });
    await act(async () => { input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })); });
    expect(api.launch).toHaveBeenCalledWith("r2", "quick_launch", "p1");
    expect(host.querySelector(".quick-launch")).toBeNull();
  });

  it("shows a clear warning when native hotkey registration fails", async () => {
    api.quickLaunchHotkeyStatus.mockResolvedValue({ registered: false, shortcut: "Alt+Space", errorCode: "hotkey_conflict" });
    await act(async () => { root.render(createElement(App)); });
    await act(async () => { await Promise.resolve(); });
    expect(host.textContent).toContain("Alt+Space could not be registered");
  });

  it("renders only the standalone quick-launch window and resets it from the native event", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    let toggle: (() => void) | undefined;
    api.onQuickLaunchToggle.mockImplementation(async (handler: () => void) => { toggle = handler; return () => undefined; });
    api.quickLaunchHotkeyStatus.mockResolvedValue({ registered: false, shortcut: "Alt+Space", errorCode: "hotkey_conflict" });
    await act(async () => { root.render(createElement(App)); await new Promise((resolve) => setTimeout(resolve, 15)); });
    expect(host.querySelector(".app-shell")).toBeNull();
    expect(host.querySelector(".quick-launch.in-window")).not.toBeNull();
    expect(host.textContent).toContain("could not be registered");
    const input = host.querySelector<HTMLInputElement>(".quick-launch-search input")!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, "old query"); input.dispatchEvent(new Event("input", { bubbles: true })); });
    await act(async () => { toggle?.(); });
    expect(host.querySelector<HTMLInputElement>(".quick-launch-search input")?.value).toBe("");
  });

  it("restores each profile's own resource ordering when switching", async () => {
    api.resources.mockImplementation(async (profileId: string) => profileId === "p2" ? [...resources].reverse() : resources);
    api.layout.mockImplementation(async (profileId: string) => profileId === "p2" ? ({ profileId, surface: "dashboard", version: 1, cards: [
      { resourceId: "r2", x: 0, y: 0, w: 4, h: 1, type: "icon" }, { resourceId: "r1", x: 4, y: 0, w: 4, h: 1, type: "icon" },
    ] }) : null);
    await act(async () => { root.render(createElement(App)); });
    const selector = host.querySelector<HTMLSelectElement>(".profile-switch")!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!.call(selector, "p2"); selector.dispatchEvent(new Event("change", { bubbles: true })); });
    expect(host.querySelector(".resource-card h3")?.textContent).toBe("Calendar");
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!.call(selector, "p1"); selector.dispatchEvent(new Event("change", { bubbles: true })); });
    expect(host.querySelector(".resource-card h3")?.textContent).toBe("Roadmap.xlsx");
  });
});
