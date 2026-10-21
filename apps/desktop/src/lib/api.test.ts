import { afterEach, describe, expect, it, vi } from "vitest";
import type { DashboardLayout } from "@sw/contracts";

const { invokeTyped } = vi.hoisted(() => ({ invokeTyped: vi.fn() }));
vi.mock("@sw/contracts", () => ({ invokeTyped }));
import { api } from "./api";

const layout: DashboardLayout = {
  profileId: "profile-work", surface: "dashboard", version: 1,
  cards: [{ resourceId: "resource-1", x: 0, y: 0, w: 6, h: 1, type: "info" }],
};

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  localStorage.clear(); invokeTyped.mockReset();
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
});
