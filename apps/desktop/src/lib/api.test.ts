import { afterEach, describe, expect, it, vi } from "vitest";
import type { DashboardLayout } from "@sw/contracts";

const { emitTyped, invokeTyped, listenTyped } = vi.hoisted(() => ({ emitTyped: vi.fn(), invokeTyped: vi.fn(), listenTyped: vi.fn() }));
vi.mock("@sw/contracts", () => ({ emitTyped, invokeTyped, listenTyped }));
import { api } from "./api";

const layout: DashboardLayout = {
  profileId: "profile-work", surface: "dashboard", version: 1,
  cards: [{ resourceId: "resource-1", x: 0, y: 0, w: 6, h: 1, type: "info" }],
};

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  localStorage.clear(); emitTyped.mockReset(); invokeTyped.mockReset(); listenTyped.mockReset();
});

describe("layout API", () => {
  it("reads and writes profile-scoped browser layouts as a fallback", async () => {
    expect(await api.layout("profile-work")).toBeNull();
    await api.saveLayout(layout);
    expect(localStorage.getItem("swl-layout-profile-work-dashboard")).toBe(JSON.stringify(layout));
    expect(await api.layout("profile-work")).toEqual(layout);
    expect(await api.layout("profile-personal")).toBeNull();
    const dockLayout = { ...layout, surface: "dock" as const };
    await api.saveLayout(dockLayout);
    expect(await api.layout("profile-work", "dock")).toEqual(dockLayout);
    expect(localStorage.getItem("swl-layout-profile-work-dock")).toBe(JSON.stringify(dockLayout));
    expect(invokeTyped).not.toHaveBeenCalled();
  });

  it("loads and saves layouts through typed native commands", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    invokeTyped.mockResolvedValueOnce(layout).mockResolvedValueOnce(layout);
    expect(await api.layout("profile-work")).toEqual(layout);
    expect(invokeTyped).toHaveBeenNthCalledWith(1, "layout_get", { profileId: "profile-work", surface: "dashboard" });
    expect(await api.saveLayout(layout)).toEqual(layout);
    expect(invokeTyped).toHaveBeenNthCalledWith(2, "layout_save", { layout });
    expect(localStorage.length).toBe(0);
  });

  it("uses the dock surface for profile-scoped Dock pins", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    const dockLayout = { ...layout, surface: "dock" as const };
    invokeTyped.mockResolvedValueOnce(dockLayout).mockResolvedValueOnce(dockLayout);
    expect(await api.layout("profile-work", "dock")).toEqual(dockLayout);
    expect(invokeTyped).toHaveBeenNthCalledWith(1, "layout_get", { profileId: "profile-work", surface: "dock" });
    expect(await api.saveLayout(dockLayout)).toEqual(dockLayout);
    expect(invokeTyped).toHaveBeenNthCalledWith(2, "layout_save", { layout: dockLayout });
    expect(emitTyped).toHaveBeenCalledWith("workspace-state-changed", { profileId: "profile-work" });
  });

  it("uses typed search in Tauri and fuzzy ranking in browser preview", async () => {
    const queryResults = [{ id: "native-result" }];
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    invokeTyped.mockResolvedValueOnce(queryResults);
    expect(await api.search("plan", "profile-work", [])).toBe(queryResults);
    expect(invokeTyped).toHaveBeenCalledWith("resource_search", { query: "plan", profileId: "profile-work", limit: 20 });

    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
    const candidates = [
      { id: "later", displayName: "Project planning", aliases: [], tags: [], targets: [], favorite: false, launchCount: 2 },
      { id: "favorite", displayName: "Planner", aliases: [], tags: [], targets: [], favorite: true, launchCount: 1 },
    ] as never[];
    expect((await api.search("plan", undefined, candidates)).map((item) => item.id)).toEqual(["favorite", "later"]);
  });

  it("subscribes to the fixed native toggle and status events", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    const onToggle = vi.fn(); const onStatus = vi.fn();
    listenTyped.mockResolvedValue(() => undefined);
    await api.onQuickLaunchToggle(onToggle);
    await api.onQuickLaunchHotkeyStatus(onStatus);
    const onWorkspace = vi.fn();
    await api.onWorkspaceStateChanged(onWorkspace);
    expect(listenTyped).toHaveBeenNthCalledWith(1, "quick-launch-toggle", onToggle);
    expect(listenTyped).toHaveBeenNthCalledWith(2, "quick-launch-hotkey-status", onStatus);
    expect(listenTyped).toHaveBeenNthCalledWith(3, "workspace-state-changed", onWorkspace);
  });

  it("queries typed hotkey registration status after subscribing", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    const status = { registered: false, shortcut: "Alt+Space", errorCode: "hotkey_conflict" };
    invokeTyped.mockResolvedValue(status);
    expect(await api.quickLaunchHotkeyStatus()).toEqual(status);
    expect(invokeTyped).toHaveBeenCalledWith("quick_launch_hotkey_status");
  });

  it("sets the profile shortcut and switches profiles through typed commands", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    const status = { registered: true, shortcut: "Ctrl+Alt+Space" };
    const profile = { id: "profile-work", name: "Work" };
    invokeTyped.mockResolvedValueOnce(status).mockResolvedValueOnce(profile);
    expect(await api.setQuickLaunchHotkey("profile-work", "Ctrl+Alt+Space")).toEqual(status);
    expect(invokeTyped).toHaveBeenNthCalledWith(1, "quick_launch_hotkey_set", { profileId: "profile-work", shortcut: "Ctrl+Alt+Space" });
    expect(await api.switchProfile("profile-work")).toEqual(profile);
    expect(invokeTyped).toHaveBeenNthCalledWith(2, "profile_switch", { profileId: "profile-work" });
    expect(emitTyped).toHaveBeenCalledWith("workspace-state-changed", { profileId: "profile-work" });
  });

  it("uses browser visibility as a preview fallback", async () => {
    await api.setDockVisibility(true);
    expect(localStorage.getItem("swl-dock-visible")).toBe("true");
    await api.setDockVisibility(false);
    expect(localStorage.getItem("swl-dock-visible")).toBe("false");
  });
});

describe("profile theme API", () => {
  it("persists native theme updates through the typed command and broadcasts profile state", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    const updated = { id: "p1", name: "Work", themeId: "dark", settingsJson: JSON.stringify({ quickLaunchShortcut: "Alt+Space", accent: "#112233", density: "compact" }) };
    invokeTyped.mockResolvedValue(updated);
    expect(await api.setProfileTheme("p1", { mode: "dark", accent: "#112233", density: "compact" })).toEqual(updated);
    expect(invokeTyped).toHaveBeenCalledWith("profile_theme_set", { profileId: "p1", theme: { mode: "dark", accent: "#112233", density: "compact" } });
    expect(emitTyped).toHaveBeenCalledWith("workspace-state-changed", { profileId: "p1" });
  });

  it("keeps profile theme preferences across browser preview reloads", async () => {
    expect(await api.setProfileTheme("default", { mode: "light", accent: "#123456", density: "comfortable" })).toMatchObject({ themeId: "light" });
    expect((await api.profiles())[0]).toMatchObject({ themeId: "light", settingsJson: JSON.stringify({ accent: "#123456", density: "comfortable" }) });
    await api.setProfileTheme("default", { mode: "dark", accent: "#abcdef" });
    expect(JSON.parse((await api.profiles())[0].settingsJson)).toEqual({ accent: "#abcdef" });
  });
});

describe("metadata and analytics API", () => {
  it("calls the typed inspection and event summary commands with exact payloads", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    const metadata = [{ resourceId: "r1", targetId: "t1", inspectedUtc: "2025-01-01T00:00:00Z", availability: "available", provenance: "local-filesystem", cacheStatus: "cached" }];
    const summary = { totalLaunches: 2, bySource: { dashboard: 1, dock: 1 }, byStatus: { handed_off: 2 } };
    const events = [{ resourceId: "r1", source: "dock", status: "handed_off", requestId: "q1", launchedAtUtc: "2025-01-01T00:00:00Z" }];
    invokeTyped.mockResolvedValueOnce(metadata).mockResolvedValueOnce(summary).mockResolvedValueOnce(events);
    expect(await api.inspect("r1", true)).toEqual(metadata);
    expect(await api.analyticsSummary("p1", "2025-01-01T00:00:00Z")).toEqual(summary);
    expect(await api.analyticsEvents("p1", 42)).toEqual(events);
    expect(invokeTyped.mock.calls).toEqual([
      ["resource_inspect", { resourceId: "r1", forceRefresh: true }],
      ["analytics_summary", { profileId: "p1", sinceUtc: "2025-01-01T00:00:00Z" }],
      ["analytics_query", { profileId: "p1", limit: 42 }],
    ]);
  });

  it("keeps browser preview clearly separate from native records", async () => {
    expect(api.isNativeApp()).toBe(false);
    expect(await api.inspect("r1")).toEqual([]);
    expect(await api.analyticsSummary("p1")).toEqual({ totalLaunches: 0, bySource: {}, byStatus: {} });
    expect(await api.analyticsEvents("p1")).toEqual([]);
    expect(invokeTyped).not.toHaveBeenCalled();
  });
});

describe("profile archive API", () => {
  it("uses the frozen typed preview, export and commit boundaries", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    const preview = { profileId: "p", profileName: "Work", packageSha256: "sha256", previewToken: "preview-token", resourceCount: 3, layoutCount: 1, historyEventCount: 2, profileIdCollision: true, profileNameCollision: true, resourceIdCollisions: ["r1"], targetIdCollisions: [], resourceConflicts: [], assetCount: 0 };
    const exported = { path: "C:\\Work.swprofile", profileName: "Work", resourceCount: 3, historyEventCount: 2 };
    const imported = { profileId: "p2", profileName: "Work (2)", importedResources: 3, reusedResources: 0, importedLayouts: 1, importedHistoryEvents: 2 };
    invokeTyped.mockResolvedValueOnce(exported).mockResolvedValueOnce(preview).mockResolvedValueOnce(imported);
    expect(await api.exportProfile("p", exported.path, true)).toEqual(exported);
    expect(await api.previewProfileImport(exported.path)).toEqual(preview);
    expect(await api.commitProfileImport(exported.path, "replace", "p", preview.packageSha256, preview.previewToken)).toEqual(imported);
    expect(emitTyped).toHaveBeenCalledWith("workspace-state-changed", { profileId: "p2" });
    expect(invokeTyped.mock.calls).toEqual([
      ["profile_export", { profileId: "p", outputPath: exported.path, includeHistory: true }],
      ["profile_import_preview", { packagePath: exported.path }],
      ["profile_import_commit", { packagePath: exported.path, strategy: "replace", targetProfileId: "p", packageSha256: "sha256", previewToken: "preview-token" }],
    ]);
  });
  it("does not emulate filesystem import/export in the browser preview", async () => {
    await expect(api.exportProfile("p", "out.swprofile")).rejects.toThrow("Windows desktop app");
    await expect(api.previewProfileImport("in.swprofile")).rejects.toThrow("Windows desktop app");
    await expect(api.commitProfileImport("in.swprofile", "duplicate", undefined, "sha", "token")).rejects.toThrow("Windows desktop app");
    expect(invokeTyped).not.toHaveBeenCalled();
  });
});
