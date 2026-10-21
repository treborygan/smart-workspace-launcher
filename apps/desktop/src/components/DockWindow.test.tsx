import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DashboardLayout, QuickLaunchHotkeyStatus } from "@sw/contracts";
import type { Resource } from "../lib/contracts";
import { computeDockGeometry, createDockAutoHideScheduler, DockWindow } from "./DockWindow";

const stamp = "2026-10-21T10:00:00.000Z";
const resources: Resource[] = [
  { id: "r1", resourceType: "file", displayName: "Roadmap", defaultAction: "open", createdUtc: stamp, updatedUtc: stamp, targets: [{ id: "t1", kind: "file", canonicalUri: "C:\\Roadmap.xlsx", priority: 0, arguments: [], active: true }], tags: [], aliases: [], favorite: false, launchCount: 0 },
  { id: "r2", resourceType: "url", displayName: "Calendar", defaultAction: "open", createdUtc: stamp, updatedUtc: stamp, targets: [{ id: "t2", kind: "url", canonicalUri: "https://calendar.example", priority: 0, arguments: [], active: true }], tags: [], aliases: [], favorite: false, launchCount: 0 },
  { id: "r3", resourceType: "folder", displayName: "Archive", defaultAction: "open", createdUtc: stamp, updatedUtc: stamp, targets: [{ id: "t3", kind: "folder", canonicalUri: "C:\\Archive", priority: 0, arguments: [], active: true }], tags: [], aliases: [], favorite: false, launchCount: 0 },
];
const layout: DashboardLayout = { profileId: "p1", surface: "dock", version: 1, cards: [
  { resourceId: "r2", x: 1, y: 0, w: 1, h: 1, type: "icon" }, { resourceId: "r1", x: 0, y: 0, w: 1, h: 1, type: "icon" },
] };
let host: HTMLDivElement; let root: Root;

beforeEach(() => { localStorage.clear(); host = document.createElement("div"); document.body.append(host); root = createRoot(host); });
afterEach(async () => { await act(async () => root.unmount()); host.remove(); localStorage.clear(); });

describe("DockWindow", () => {
  it("shows profile-pinned resources in saved order and exposes placement settings", async () => {
    await act(async () => root.render(createElement(DockWindow, { profileId: "p1", profileName: "Personal", resources, layout, hotkeyStatus: null, onLaunch: vi.fn(), onClose: vi.fn(), onRefresh: vi.fn() })));
    expect([...host.querySelectorAll<HTMLButtonElement>(".dock-item")].map((button) => button.getAttribute("aria-label"))).toEqual(["Open Roadmap", "Open Calendar"]);
    expect(host.querySelector(".dock-profile")?.textContent).toBe("Personal");
    expect(host.querySelector(".dock-settings")).not.toBeNull();
  });

  it("surfaces launch failure without closing and shows hotkey conflict status", async () => {
    const onLaunch = vi.fn().mockResolvedValue({ requestId: "x", status: "failed", errorCode: "target_not_found", launchedAtUtc: stamp });
    const hotkeyStatus: QuickLaunchHotkeyStatus = { registered: false, shortcut: "Alt+Space", activeShortcut: "Ctrl+Space", errorCode: "hotkey_conflict" };
    await act(async () => root.render(createElement(DockWindow, { profileId: "p1", profileName: "Personal", resources, layout, hotkeyStatus, onLaunch, onClose: vi.fn(), onRefresh: vi.fn() })));
    await act(async () => host.querySelector<HTMLButtonElement>('[aria-label="Open Roadmap"]')!.click());
    expect(onLaunch).toHaveBeenCalledWith(resources[0]);
    expect(host.textContent).toContain("target_not_found");
    expect(host.textContent).toContain("Ctrl+Space");
  });

  it("keeps Dock preferences scoped to the selected profile", async () => {
    const props = { resources, layout, hotkeyStatus: null, onLaunch: vi.fn(), onClose: vi.fn(), onRefresh: vi.fn(), profileName: "Personal" };
    await act(async () => root.render(createElement(DockWindow, { ...props, profileId: "p1" })));
    await act(async () => host.querySelector(".dock-settings summary")!.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    const size = host.querySelector<HTMLInputElement>('.dock-settings input[type="range"]')!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(size, "64"); size.dispatchEvent(new Event("input", { bubbles: true })); size.dispatchEvent(new Event("change", { bubbles: true })); });
    await act(async () => root.render(createElement(DockWindow, { ...props, profileId: "p2" })));
    expect(host.querySelector<HTMLInputElement>('.dock-settings input[type="range"]')?.value).toBe("48");
    await act(async () => root.render(createElement(DockWindow, { ...props, profileId: "p1" })));
    expect(host.querySelector<HTMLInputElement>('.dock-settings input[type="range"]')?.value).toBe("64");
    expect(localStorage.getItem("swl-dock-preferences-p1-v1")).toContain('"iconSize":64');
    expect(localStorage.getItem("swl-dock-preferences-p2-v1")).toContain('"iconSize":48');
  });

  it("cancels delayed hide when Dock regains focus and rechecks focus before hiding", async () => {
    vi.useFakeTimers();
    const hide = vi.fn();
    const isFocused = vi.fn().mockResolvedValue(false);
    const scheduler = createDockAutoHideScheduler(hide, isFocused, 450);
    scheduler.focusChanged(false);
    scheduler.focusChanged(true);
    await vi.advanceTimersByTimeAsync(500);
    expect(hide).not.toHaveBeenCalled();
    scheduler.focusChanged(false);
    isFocused.mockResolvedValueOnce(true);
    await vi.advanceTimersByTimeAsync(500);
    expect(hide).not.toHaveBeenCalled();
    scheduler.focusChanged(false);
    await vi.advanceTimersByTimeAsync(500);
    expect(hide).toHaveBeenCalledTimes(1);
    scheduler.dispose();
    vi.useRealTimers();
  });

  it("clamps Dock size and edge position to short and offset monitor work areas", () => {
    const rightEdge = computeDockGeometry({ edge: "right", orientation: "vertical", iconSize: 72, spacing: 20, pinCount: 30, workArea: { x: -1280, y: 40, width: 1280, height: 140 } });
    expect(rightEdge).toEqual({ x: -96, y: 40, width: 96, height: 140 });
    expect(rightEdge.x).toBeGreaterThanOrEqual(-1280);
    expect(rightEdge.x + rightEdge.width).toBeLessThanOrEqual(0);
    expect(rightEdge.y + rightEdge.height).toBeLessThanOrEqual(180);

    const bottomEdge = computeDockGeometry({ edge: "bottom", orientation: "horizontal", iconSize: 72, spacing: 20, pinCount: 30, workArea: { x: 80, y: -20, width: 110, height: 75 } });
    expect(bottomEdge).toEqual({ x: 80, y: -20, width: 110, height: 75 });
    expect(bottomEdge.x + bottomEdge.width).toBeLessThanOrEqual(190);
    expect(bottomEdge.y + bottomEdge.height).toBeLessThanOrEqual(55);
  });
});
