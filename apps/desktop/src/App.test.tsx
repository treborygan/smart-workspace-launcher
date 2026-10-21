import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Profile, Resource } from "./lib/contracts";

const { api, windowState } = vi.hoisted(() => ({
  api: { resources: vi.fn(), profiles: vi.fn(), smartCollections: vi.fn(), saveSmartCollection: vi.fn(), deleteSmartCollection: vi.fn(), dynamicShortcuts: vi.fn(), saveDynamicShortcut: vi.fn(), deleteDynamicShortcut: vi.fn(), workspaceSequences: vi.fn(), saveWorkspaceSequence: vi.fn(), deleteWorkspaceSequence: vi.fn(), runWorkspaceSequence: vi.fn(), layout: vi.fn(), saveLayout: vi.fn(), search: vi.fn(), onQuickLaunchToggle: vi.fn(), onQuickLaunchHotkeyStatus: vi.fn(), onWorkspaceStateChanged: vi.fn(), quickLaunchHotkeyStatus: vi.fn(), setQuickLaunchHotkey: vi.fn(), setProfileTheme: vi.fn(), switchProfile: vi.fn(), hideCurrentWindow: vi.fn(), create: vi.fn(), update: vi.fn(), remove: vi.fn(), launch: vi.fn(), inspect: vi.fn(), analyticsSummary: vi.fn(), analyticsEvents: vi.fn(), isNativeApp: vi.fn() },
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
  api.inspect.mockResolvedValue([]); api.analyticsSummary.mockResolvedValue({ totalLaunches: 0, bySource: {}, byStatus: {} }); api.analyticsEvents.mockResolvedValue([]); api.isNativeApp.mockReturnValue(false); api.smartCollections.mockResolvedValue([]); api.saveSmartCollection.mockImplementation(async (collection) => collection); api.deleteSmartCollection.mockResolvedValue(undefined); api.dynamicShortcuts.mockResolvedValue([]); api.saveDynamicShortcut.mockImplementation(async (shortcut) => shortcut); api.deleteDynamicShortcut.mockResolvedValue(undefined); api.workspaceSequences.mockResolvedValue([]); api.saveWorkspaceSequence.mockImplementation(async (sequence) => sequence); api.deleteWorkspaceSequence.mockResolvedValue(undefined); api.runWorkspaceSequence.mockResolvedValue({ sequenceId: "s1", status: "completed", totalSteps: 1, completedSteps: 1, steps: [] });
  api.setProfileTheme.mockImplementation(async (id: string, theme: { mode: string; accent: string; density: string }) => ({ ...profiles.find((profile) => profile.id === id)!, themeId: theme.mode, settingsJson: JSON.stringify({ ...JSON.parse(profiles.find((profile) => profile.id === id)!.settingsJson), accent: theme.accent, density: theme.density }) }));
  localStorage.clear(); api.resources.mockResolvedValue(resources); api.profiles.mockResolvedValue(profiles); api.layout.mockResolvedValue(null); api.saveLayout.mockImplementation(async (layout) => layout);
  api.search.mockImplementation(async (query: string, _profileId: string, fallback: Resource[]) => query ? fallback.filter((resource) => `${resource.displayName} ${resource.description}`.toLowerCase().includes(query.toLowerCase())) : fallback);
  windowState.label = "quick-launch";
  api.onQuickLaunchToggle.mockResolvedValue(() => undefined); api.onQuickLaunchHotkeyStatus.mockResolvedValue(() => undefined); api.onWorkspaceStateChanged.mockResolvedValue(() => undefined);
  api.quickLaunchHotkeyStatus.mockResolvedValue({ registered: true, shortcut: "Alt+Space" });
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
});
afterEach(async () => { await act(async () => root.unmount()); host.remove(); Reflect.deleteProperty(window, "__TAURI_INTERNALS__"); Reflect.deleteProperty(window, "matchMedia"); vi.clearAllMocks(); });

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

  it("restores each profile's saved theme and follows system appearance changes", async () => {
    let savedProfiles = profiles.map((profile, index) => index === 1
      ? { ...profile, themeId: "dark", settingsJson: JSON.stringify({ accent: "#224466", density: "compact" }) }
      : { ...profile, themeId: "system", settingsJson: JSON.stringify({ accent: "#116677", density: "balanced" }) });
    api.profiles.mockImplementation(async () => savedProfiles);
    api.setProfileTheme.mockImplementation(async (id: string, theme: { mode: string; accent: string; density?: string }) => {
      const previous = savedProfiles.find((profile) => profile.id === id)!;
      const updated = { ...previous, themeId: theme.mode, settingsJson: JSON.stringify({ ...JSON.parse(previous.settingsJson), accent: theme.accent, ...(theme.density ? { density: theme.density } : {}) }) };
      savedProfiles = savedProfiles.map((profile) => profile.id === id ? updated : profile);
      return updated;
    });
    let mediaListener: ((event: MediaQueryListEvent) => void) | undefined;
    const mediaQuery = { matches: false, addEventListener: (_event: string, listener: (event: MediaQueryListEvent) => void) => { mediaListener = listener; }, removeEventListener: vi.fn() };
    Object.defineProperty(window, "matchMedia", { configurable: true, value: vi.fn(() => mediaQuery) });
    windowState.label = "browser";
    await act(async () => { root.render(createElement(App)); await new Promise((resolve) => setTimeout(resolve, 15)); });
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe("#116677");
    mediaQuery.matches = true;
    await act(async () => { mediaListener?.({ matches: true } as MediaQueryListEvent); });
    expect(document.documentElement.dataset.theme).toBe("dark");

    const settings = [...host.querySelectorAll("button")].find((button) => button.textContent?.includes("Settings"))!;
    await act(async () => { settings.click(); });
    const mode = host.querySelector<HTMLSelectElement>('[aria-label="Theme mode"]')!;
    await act(async () => { mode.value = "dark"; mode.dispatchEvent(new Event("change", { bubbles: true })); });
    const accent = host.querySelector<HTMLInputElement>('[aria-label="Custom accent color"]')!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(accent, "#aa3344"); accent.dispatchEvent(new Event("input", { bubbles: true })); });
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent?.includes("Save appearance"))!.click(); await new Promise((resolve) => setTimeout(resolve, 5)); });

    const selector = host.querySelector<HTMLSelectElement>(".profile-switch")!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!.call(selector, "p2"); selector.dispatchEvent(new Event("change", { bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 15)); });
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe("#224466");
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!.call(selector, "p1"); selector.dispatchEvent(new Event("change", { bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 15)); });
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe("#aa3344");
    expect(themePreferenceFrom(savedProfiles, "p1")).toMatchObject({ mode: "dark", accent: "#aa3344" });
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

  it("keeps saved-query shortcuts isolated per profile and wires collection create/select", async () => {
    const p1Collections = [{ id: "sc-work", profileId: "p1", name: "Work files", filters: { resourceType: "file" as const } }];
    const p2Collections = [{ id: "sc-web", profileId: "p2", name: "Web links", filters: { resourceType: "url" as const } }];
    api.smartCollections.mockImplementation(async (profileId: string) => profileId === "p2" ? p2Collections : p1Collections);
    api.resources.mockImplementation(async () => resources);
    api.saveSmartCollection.mockImplementation(async (collection) => collection);
    await act(async () => { root.render(createElement(App)); await new Promise((resolve) => setTimeout(resolve, 15)); });
    const rows = () => [...host.querySelectorAll<HTMLElement>(".resource-card")];
    await act(async () => { host.querySelector<HTMLButtonElement>('[aria-label="Work files"]')?.click(); });
    expect(rows().map((card) => card.querySelector("h3")?.textContent)).toEqual(["Roadmap.xlsx"]);

    const profile = host.querySelector<HTMLSelectElement>(".profile-switch")!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!.call(profile, "p2"); profile.dispatchEvent(new Event("change", { bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 15)); });
    expect(rows().map((card) => card.querySelector("h3")?.textContent)).toEqual(["Roadmap.xlsx", "Calendar"]);
    await act(async () => { host.querySelector<HTMLButtonElement>('[aria-label="Web links"]')?.click(); });
    expect(rows().map((card) => card.querySelector("h3")?.textContent)).toEqual(["Calendar"]);
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!.call(profile, "p1"); profile.dispatchEvent(new Event("change", { bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 15)); });
    expect(rows().map((card) => card.querySelector("h3")?.textContent)).toEqual(["Roadmap.xlsx"]);

    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent === "Smart Collections")!.click(); });
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent === "New collection")!.click(); });
    const name = host.querySelector<HTMLInputElement>('[aria-label="Collection name"]')!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(name, "PDF files"); name.dispatchEvent(new Event("input", { bubbles: true })); });
    const type = host.querySelector<HTMLSelectElement>('[aria-label="Resource type filter"]')!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!.call(type, "file"); type.dispatchEvent(new Event("change", { bubbles: true })); });
    await act(async () => { host.querySelector<HTMLFormElement>(".smart-collection-form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })); await new Promise((resolve) => setTimeout(resolve, 5)); });
    expect(api.saveSmartCollection).toHaveBeenCalledWith(expect.objectContaining({ profileId: "p1", name: "PDF files", filters: { resourceType: "file" } }));
    expect(host.textContent).toContain("PDF files");
  });

  it("manages a saved shortcut and launches only its saved resource target from Quick Launch", async () => {
    const saved = { id: "ds-report", profileId: "p1", name: "Open roadmap backup", resourceId: "r1", targetId: "t1" };
    api.saveDynamicShortcut.mockResolvedValue(saved);
    api.launch.mockResolvedValue({ requestId: "launch-ds", status: "handed_off", launchedAtUtc: timestamp });
    await act(async () => { root.render(createElement(App)); await new Promise((resolve) => setTimeout(resolve, 15)); });
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent === "Quick Launch shortcuts")!.click(); });
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent === "New shortcut")!.click(); });
    const name = host.querySelector<HTMLInputElement>('[aria-label="Shortcut name"]')!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(name, "Open roadmap backup"); name.dispatchEvent(new Event("input", { bubbles: true })); });
    const target = host.querySelector<HTMLSelectElement>('[aria-label="Shortcut target"]')!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!.call(target, "t1"); target.dispatchEvent(new Event("change", { bubbles: true })); });
    await act(async () => { host.querySelector(".dynamic-shortcuts-manager form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })); await new Promise((resolve) => setTimeout(resolve, 5)); });
    expect(api.saveDynamicShortcut).toHaveBeenCalledWith(expect.objectContaining({ profileId: "p1", resourceId: "r1", targetId: "t1", name: "Open roadmap backup" }));

    await act(async () => { window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 20)); });
    const quickShortcut = host.querySelector<HTMLButtonElement>('[aria-label="Saved shortcut Open roadmap backup opens Roadmap.xlsx"]');
    expect(quickShortcut).not.toBeNull();
    await act(async () => { quickShortcut!.click(); await new Promise((resolve) => setTimeout(resolve, 10)); });
    expect(api.launch).toHaveBeenCalledWith("r1", "quick_launch", "p1", "t1");
    expect(host.querySelector(".quick-launch")).toBeNull();
  });

  it("loads only the active profile's dynamic shortcuts into Quick Launch", async () => {
    api.dynamicShortcuts.mockImplementation(async (profileId: string) => profileId === "p2"
      ? [{ id: "ds-p2", profileId, name: "Work calendar", resourceId: "r2" }]
      : [{ id: "ds-p1", profileId, name: "Personal roadmap", resourceId: "r1" }]);
    await act(async () => { root.render(createElement(App)); await new Promise((resolve) => setTimeout(resolve, 15)); });
    const open = async () => act(async () => { window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 15)); });
    await open();
    expect(host.querySelector('[aria-label="Saved shortcut Personal roadmap opens Roadmap.xlsx"]')).not.toBeNull();
    expect(host.querySelector('[aria-label="Saved shortcut Work calendar opens Calendar"]')).toBeNull();
    await act(async () => { host.querySelector<HTMLInputElement>(".quick-launch-search input")!.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })); });
    const profile = host.querySelector<HTMLSelectElement>(".profile-switch")!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!.call(profile, "p2"); profile.dispatchEvent(new Event("change", { bubbles: true })); await new Promise((resolve) => setTimeout(resolve, 15)); });
    await open();
    expect(host.querySelector('[aria-label="Saved shortcut Work calendar opens Calendar"]')).not.toBeNull();
    expect(host.querySelector('[aria-label="Saved shortcut Personal roadmap opens Roadmap.xlsx"]')).toBeNull();
  });

  it("opens workspace sequences for the active profile", async () => {
    await act(async () => { root.render(createElement(App)); await new Promise((resolve) => setTimeout(resolve, 15)); });
    await act(async () => { const button = [...host.querySelectorAll(".nav-item")].find((item) => item.textContent?.includes("Workspace sequences")) as HTMLButtonElement; button.click(); });
    expect(host.querySelector(".workspace-sequences-manager h1")?.textContent).toBe("Workspace sequences");
    expect(api.workspaceSequences).toHaveBeenCalledWith("p1");
  });
});

function themePreferenceFrom(items: Profile[], profileId: string) {
  const profile = items.find((item) => item.id === profileId)!;
  return { mode: profile.themeId, ...JSON.parse(profile.settingsJson) };
}
