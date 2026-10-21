import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Profile } from "../lib/contracts";
import { DEFAULT_THEME, resolveAppearance, themeForProfile } from "../lib/profileTheme";
import { ProfileThemeSettings } from "./ProfileThemeSettings";
let host: HTMLDivElement; let root: Root; const onSave = vi.fn();
const profile: Profile = { id: "p", name: "Work", themeId: "dark", settingsJson: JSON.stringify({ accent: "#e24488", density: "compact", quickLaunchShortcut: "Ctrl+Space" }), createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z" };
beforeEach(() => { host = document.createElement("div"); document.body.append(host); root = createRoot(host); onSave.mockReset(); onSave.mockResolvedValue(undefined); });
afterEach(async () => { await act(async () => root.unmount()); host.remove(); });
describe("profile theme settings", () => {
  it("reads profile mode, accent and density with safe defaults", () => {
    expect(themeForProfile(profile)).toEqual({ mode: "dark", accent: "#e24488", density: "compact" });
    expect(themeForProfile({ ...profile, themeId: "bad", settingsJson: "invalid" })).toEqual(DEFAULT_THEME);
    expect(resolveAppearance("system", true)).toBe("dark"); expect(resolveAppearance("system", false)).toBe("light");
    expect(resolveAppearance("light", true)).toBe("light");
  });
  it("saves explicit profile mode, custom accent and density", async () => {
    await act(async () => root.render(createElement(ProfileThemeSettings, { value: { mode: "system", accent: "#5755d9", density: "balanced" }, onSave })));
    const mode = host.querySelector<HTMLSelectElement>('[aria-label="Theme mode"]')!;
    await act(async () => { mode.value = "dark"; mode.dispatchEvent(new Event("change", { bubbles: true })); });
    const accent = host.querySelector<HTMLInputElement>('[aria-label="Custom accent color"]')!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(accent, "#1188aa"); accent.dispatchEvent(new Event("input", { bubbles: true })); });
    const density = host.querySelector<HTMLSelectElement>('[aria-label="Density"]')!;
    await act(async () => { density.value = "comfortable"; density.dispatchEvent(new Event("change", { bubbles: true })); });
    await act(async () => { host.querySelector("form")!.requestSubmit(); await Promise.resolve(); });
    expect(onSave).toHaveBeenCalledWith({ mode: "dark", accent: "#1188aa", density: "comfortable" });
  });
});
