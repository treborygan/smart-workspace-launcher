import { afterEach, describe, expect, it, vi } from "vitest";
import type { DashboardLayout } from "@sw/contracts";

const { invokeTyped, listenTyped } = vi.hoisted(() => ({ invokeTyped: vi.fn(), listenTyped: vi.fn() }));
vi.mock("@sw/contracts", () => ({ invokeTyped, listenTyped }));
import { api } from "./api";

const layout: DashboardLayout = {
  profileId: "profile-work", surface: "dashboard", version: 1,
  cards: [{ resourceId: "resource-1", x: 0, y: 0, w: 6, h: 1, type: "info" }],
};

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  localStorage.clear(); invokeTyped.mockReset(); listenTyped.mockReset();
});

describe("layout API", () => {
  it("reads and writes profile-scoped browser layouts as a fallback", async () => {
    expect(await api.layout("profile-work")).toBeNull();
    await api.saveLayout(layout);
    expect(localStorage.getItem("swl-layout-profile-work-dashboard")).toBe(JSON.stringify(layout));
    expect(await api.layout("profile-work")).toEqual(layout);
    expect(await api.layout("profile-personal")).toBeNull();
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
    expect(listenTyped).toHaveBeenNthCalledWith(1, "quick-launch-toggle", onToggle);
    expect(listenTyped).toHaveBeenNthCalledWith(2, "quick-launch-hotkey-status", onStatus);
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
  });
});
