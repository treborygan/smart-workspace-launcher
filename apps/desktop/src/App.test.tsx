import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Profile, Resource } from "./lib/contracts";

const { api, windowState } = vi.hoisted(() => ({
  api: { resources: vi.fn(), profiles: vi.fn(), layout: vi.fn(), saveLayout: vi.fn(), search: vi.fn(), onQuickLaunchToggle: vi.fn(), onQuickLaunchHotkeyStatus: vi.fn(), onWorkspaceStateChanged: vi.fn(), quickLaunchHotkeyStatus: vi.fn(), setQuickLaunchHotkey: vi.fn(), setProfileTheme: vi.fn(), switchProfile: vi.fn(), hideCurrentWindow: vi.fn(), create: vi.fn(), update: vi.fn(), remove: vi.fn(), launch: vi.fn(), inspect: vi.fn(), analyticsSummary: vi.fn(), analyticsEvents: vi.fn(), isNativeApp: vi.fn() },
  windowState: { label: "quick-launch" },
}));
vi.mock("./lib/api", () => ({ api }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ label: windowState.label, onFocusChanged: async () => () => undefined }) }));
import App, { columnsForViewport } from "./App";

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
  api.inspect.mockResolvedValue([]); api.analyticsSummary.mockResolvedValue({ totalLaunches: 0, bySource: {}, byStatus: {} }); api.analyticsEvents.mockResolvedValue([]); api.isNativeApp.mockReturnValue(false);
  api.setProfileTheme.mockImplementation(async (id: string, theme: { mode: string; accent: string; density: string }) => ({ ...profiles.find((profile) => profile.id === id)!, themeId: theme.mode, settingsJson: JSON.stringify({ ...JSON.parse(profiles.find((profile) => profile.id === id)!.settingsJson), accent: theme.accent, density: theme.density }) }));
  localStorage.clear(); api.resources.mockResolvedValue(resources); api.profiles.mockResolvedValue(profiles); api.layout.mockResolvedValue(null); api.saveLayout.mockImplementation(async (layout) => layout);
  api.search.mockImplementation(async (query: string, _profileId: string, fallback: Resource[]) => query ? fallback.filter((resource) => `${resource.displayName} ${resource.description}`.toLowerCase().includes(query.toLowerCase())) : fallback);
  windowState.label = "quick-launch";
  api.onQuickLaunchToggle.mockResolvedValue(() => undefined); api.onQuickLaunchHotkeyStatus.mockResolvedValue(() => undefined); api.onWorkspaceStateChanged.mockResolvedValue(() => undefined);
  api.quickLaunchHotkeyStatus.mockResolvedValue({ registered: true, shortcut: "Alt+Space" });
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
});
afterEach(async () => { await act(async () => root.unmount()); host.remove(); Reflect.deleteProperty(window, "__TAURI_INTERNALS__"); vi.clearAllMocks(); });

describe("workspace dashboard", () => {
  it("chooses grid spans that fit each responsive breakpoint", () => {
    expect(columnsForViewport(1440)).toBe(12);
    expect(columnsForViewport(1024)).toBe(6);
    expect(columnsForViewport(390)).toBe(4);
  });

  it("offers keyboard card reordering and persists card height", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    windowState.label = "main";
    await act(async () => { root.render(createElement(App)); await new Promise((resolve) => setTimeout(resolve, 15)); });
    const moveEarlier = [...host.querySelectorAll("button")].find((button) => button.getAttribute("aria-label") === "Move Calendar earlier")!;
    await act(async () => { moveEarlier.click(); await new Promise((resolve) => setTimeout(resolve, 5)); });
    let saved = api.saveLayout.mock.calls.at(-1)?.[0];
    expect(saved.cards[0].resourceId).toBe("r2");
    const calendar = [...host.querySelectorAll<HTMLElement>(".resource-card")].find((card) => card.textContent?.includes("Calendar"))!;
    await act(async () => { calendar.click(); });
    const tall = [...host.querySelectorAll("button")].find((button) => button.textContent === "Tall")!;
    await act(async () => { tall.click(); await new Promise((resolve) => setTimeout(resolve, 5)); });
    saved = api.saveLayout.mock.calls.at(-1)?.[0];
    expect(saved.cards.find((card: { resourceId: string }) => card.resourceId === "r2").h).toBe(2);
  });

  it("loads the saved resources and exposes the inspector", async () => {
    await act(async () => { root.render(createElement(App)); });
    expect(host.textContent).toContain("Roadmap.xlsx");
    expect(host.textContent).toContain("Pinned resources");
    expect(host.textContent).toContain("Inspector");
  });

  it("refreshes main resources and layouts after a workspace event from another surface", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    windowState.label = "main";
    let workspaceChanged: ((payload: { profileId: string }) => void) | undefined;
    api.onWorkspaceStateChanged.mockImplementation(async (handler: (payload: { profileId: string }) => void) => { workspaceChanged = handler; return () => undefined; });
    const refreshed = resources.map((resource) => resource.id === "r1" ? { ...resource, launchCount: 7, lastLaunchedUtc: "2026-10-21T11:00:00.000Z" } : resource);
    api.resources.mockResolvedValueOnce(resources).mockResolvedValue(refreshed);
    await act(async () => { root.render(createElement(App)); await new Promise((resolve) => setTimeout(resolve, 20)); });
    const roadmapCard = () => [...host.querySelectorAll<HTMLElement>(".resource-card")].find((card) => card.textContent?.includes("Roadmap.xlsx"));
    await act(async () => roadmapCard()?.click());
    const roadmapCount = () => [...host.querySelectorAll<HTMLElement>(".info-row")].find((row) => row.textContent?.includes("Launches"))?.querySelector("b")?.textContent;
    expect(roadmapCount()).toBe("2");
    await act(async () => { workspaceChanged?.({ profileId: "p1" }); await new Promise((resolve) => setTimeout(resolve, 20)); });
    expect(api.resources).toHaveBeenLastCalledWith("p1");
    expect(roadmapCount()).toBe("7");
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
    await act(async () => { window.dispatchEvent(new KeyboardEvent("keydown", { code: "Space", key: " ", altKey: true, bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 100)); });
    const input = host.querySelector<HTMLInputElement>(".quick-launch-search input")!;
    expect(document.activeElement).toBe(input);
    await act(async () => { input.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })); });
    await act(async () => { input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })); });
    expect(api.launch).toHaveBeenCalledWith("r2", "quick_launch", "p1");
    expect(host.querySelector(".quick-launch")).toBeNull();
  });

  it("keeps Quick Launch open and shows an actionable error when launch fails", async () => {
    api.launch.mockRejectedValue(new Error("The target file was not found. Check its location and try again."));
    await act(async () => { root.render(createElement(App)); });
    await act(async () => { window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 15)); });
    const input = host.querySelector<HTMLInputElement>(".quick-launch-search input")!;
    await act(async () => { input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 15)); });
    expect(host.querySelector(".quick-launch")).not.toBeNull();
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("target file was not found");
  });

  it("shows a clear warning when native hotkey registration fails", async () => {
    api.quickLaunchHotkeyStatus.mockResolvedValue({ registered: false, shortcut: "Alt+Space", errorCode: "hotkey_conflict" });
    await act(async () => { root.render(createElement(App)); });
    await act(async () => { await Promise.resolve(); });
    expect(host.textContent).toContain("Alt+Space could not be registered");
  });

  it("configures a profile Quick Launch shortcut from Settings", async () => {
    api.setQuickLaunchHotkey.mockResolvedValue({ registered: true, shortcut: "Ctrl+Shift+Space", activeShortcut: "Ctrl+Shift+Space" });
    await act(async () => { root.render(createElement(App)); });
    const settings = [...host.querySelectorAll("button")].find((button) => button.textContent?.includes("Settings"))!;
    await act(async () => { settings.click(); });
    const input = host.querySelector<HTMLInputElement>("#quick-launch-shortcut")!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, "Ctrl+Shift+Space"); input.dispatchEvent(new Event("input", { bubbles: true })); });
    await act(async () => { host.querySelector<HTMLFormElement>(".shortcut-settings")!.requestSubmit(); await new Promise((resolve) => setTimeout(resolve, 10)); });
    expect(api.setQuickLaunchHotkey).toHaveBeenCalledWith("p1", "Ctrl+Shift+Space");
    expect(host.textContent).toContain("Quick Launch shortcut set to Ctrl+Shift+Space");
  });

  it("saves light/dark/system mode, custom accent and density to the active profile", async () => {
    windowState.label = "browser";
    await act(async () => { root.render(createElement(App)); });
    const settings = [...host.querySelectorAll("button")].find((button) => button.textContent?.includes("Settings"))!;
    await act(async () => { settings.click(); });
    const mode = host.querySelector<HTMLSelectElement>('[aria-label="Theme mode"]')!;
    await act(async () => { mode.value = "dark"; mode.dispatchEvent(new Event("change", { bubbles: true })); });
    const color = host.querySelector<HTMLInputElement>('[aria-label="Custom accent color"]')!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(color, "#1188aa"); color.dispatchEvent(new Event("input", { bubbles: true })); });
    const density = host.querySelector<HTMLSelectElement>('[aria-label="Density"]')!;
    await act(async () => { density.value = "comfortable"; density.dispatchEvent(new Event("change", { bubbles: true })); });
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent?.includes("Save appearance"))!.click(); await Promise.resolve(); });
    expect(api.setProfileTheme).toHaveBeenCalledWith("p1", { mode: "dark", accent: "#1188aa", density: "comfortable" });
    expect(host.textContent).toContain("Appearance saved for Personal");
  });

  it("shows each profile's requested shortcut after conflicts and preserves another profile's unsaved draft", async () => {
    let selectedProfileId = "p1";
    api.profiles.mockResolvedValue(profiles.map((profile) => profile.id === "p2" ? { ...profile, settingsJson: JSON.stringify({ quickLaunchShortcut: "Ctrl+Space" }) } : profile));
    api.switchProfile.mockImplementation(async (profileId: string) => { selectedProfileId = profileId; return profiles.find((profile) => profile.id === profileId); });
    api.quickLaunchHotkeyStatus.mockImplementation(async () => selectedProfileId === "p1"
      ? { registered: false, shortcut: "Alt+Space", activeShortcut: "Ctrl+Alt+Space", errorCode: "hotkey_conflict" }
      : { registered: true, shortcut: "Ctrl+Space" });
    await act(async () => { root.render(createElement(App)); await new Promise((resolve) => setTimeout(resolve, 15)); });
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent?.includes("Settings"))!.click(); });
    const input = host.querySelector<HTMLInputElement>("#quick-launch-shortcut")!;
    expect(input.value).toBe("Alt+Space");
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, "Ctrl+Shift+Space"); input.dispatchEvent(new Event("input", { bubbles: true })); });
    const selector = host.querySelector<HTMLSelectElement>(".profile-switch")!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!.call(selector, "p2"); selector.dispatchEvent(new Event("change", { bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 20)); });
    expect(host.querySelector<HTMLInputElement>("#quick-launch-shortcut")?.value).toBe("Ctrl+Space");
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!.call(selector, "p1"); selector.dispatchEvent(new Event("change", { bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 20)); });
    expect(host.querySelector<HTMLInputElement>("#quick-launch-shortcut")?.value).toBe("Ctrl+Shift+Space");
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
    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 10)); });
    expect(host.querySelector<HTMLInputElement>(".quick-launch-search input")?.value).toBe("");
    expect(api.resources).toHaveBeenCalledTimes(2);
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
    expect(api.switchProfile).toHaveBeenCalledWith("p1");
    expect(api.switchProfile).toHaveBeenCalledWith("p2");
  });
});
