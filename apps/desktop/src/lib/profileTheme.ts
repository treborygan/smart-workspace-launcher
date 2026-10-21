import type { Profile } from "./contracts";

export interface ThemePreference {
  mode: "system" | "light" | "dark";
  accent: string;
  density: "compact" | "balanced" | "comfortable";
}
export const DEFAULT_THEME: ThemePreference = { mode: "system", accent: "#5755d9", density: "balanced" };
export function themeForProfile(profile: Pick<Profile, "id" | "themeId" | "settingsJson">): ThemePreference {
  let settings: Record<string, unknown> = {};
  try { settings = JSON.parse(profile.settingsJson || "{}") as Record<string, unknown>; } catch { /* use safe defaults */ }
  const mode = profile.themeId === "light" || profile.themeId === "dark" ? profile.themeId : "system";
  const accent = typeof settings.accent === "string" && /^#[0-9a-f]{6}$/i.test(settings.accent) ? settings.accent : DEFAULT_THEME.accent;
  const density = ["compact", "balanced", "comfortable"].includes(String(settings.density)) ? settings.density as ThemePreference["density"] : DEFAULT_THEME.density;
  return { mode, accent, density };
}
export function resolveAppearance(mode: ThemePreference["mode"], systemPrefersDark: boolean) {
  return mode === "system" ? systemPrefersDark ? "dark" : "light" : mode;
}
