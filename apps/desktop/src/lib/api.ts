import { emitTyped, invokeTyped, listenTyped } from "@sw/contracts";
import type { AppEventMap, DashboardLayout, LaunchSource, QuickLaunchHotkeyStatus, ResourcePatch } from "@sw/contracts";
import type { LaunchResult, Profile, Resource, ResourceDraft, ResourceTarget } from "./contracts";

const STORAGE_KEY = "swl-ui-demo-resources-v1";
const isNative = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const activeProfileId = () => typeof localStorage === "undefined" ? undefined : localStorage.getItem("swl-active-profile") || undefined;
async function notifyWorkspaceChanged(profileId?: string) {
  const currentProfileId = profileId || activeProfileId();
  if (isNative() && currentProfileId) await emitTyped("workspace-state-changed", { profileId: currentProfileId });
}

const now = new Date().toISOString();
const demoResources: Resource[] = [
  { id: "a1", resourceType: "file", displayName: "Design review", description: "Product design and feedback", defaultAction: "open", createdUtc: now, updatedUtc: now, category: "Work", favorite: true, launchCount: 18, targets: [{ id: "t1", kind: "file", canonicalUri: "C:\\Workspace\\Design review", priority: 0, arguments: [], active: true }], tags: [], aliases: [] },
  { id: "a2", resourceType: "file", displayName: "Q3 Planning.xlsx", description: "Quarterly planning workbook", defaultAction: "open", createdUtc: now, updatedUtc: now, category: "Work", favorite: true, launchCount: 12, targets: [{ id: "t2", kind: "file", canonicalUri: "C:\\Workspace\\Q3 Planning.xlsx", priority: 0, arguments: [], active: true }], tags: [], aliases: [] },
  { id: "a3", resourceType: "url", displayName: "Notion", description: "Team workspace", defaultAction: "open", createdUtc: now, updatedUtc: now, category: "Web", favorite: true, launchCount: 9, targets: [{ id: "t3", kind: "url", canonicalUri: "https://www.notion.so", priority: 0, arguments: [], active: true }], tags: [], aliases: [] },
  { id: "a4", resourceType: "folder", displayName: "Shared drive", description: "Shared team files", defaultAction: "open", createdUtc: now, updatedUtc: now, category: "Work", favorite: true, launchCount: 7, targets: [{ id: "t4", kind: "folder", canonicalUri: "Z:\\Team", priority: 0, arguments: [], active: true }], tags: [], aliases: [] },
  { id: "a5", resourceType: "url", displayName: "Calendar", description: "Your schedule", defaultAction: "open", createdUtc: now, updatedUtc: now, category: "Web", favorite: true, launchCount: 22, targets: [{ id: "t5", kind: "url", canonicalUri: "https://calendar.google.com", priority: 0, arguments: [], active: true }], tags: [], aliases: [] },
  { id: "a6", resourceType: "app", displayName: "Microsoft Excel", description: "Spreadsheets", defaultAction: "open", createdUtc: now, updatedUtc: now, category: "Apps", favorite: false, launchCount: 31, targets: [{ id: "t6", kind: "app", canonicalUri: "C:\\Program Files\\Microsoft Office\\root\\Office16\\EXCEL.EXE", priority: 0, arguments: [], active: true }], tags: [], aliases: [] },
];

function readDemo(): Resource[] {
  try { return JSON.parse(localStorage.getItem(STORAGE_KEY) || "null") || demoResources; }
  catch { return demoResources; }
}
function writeDemo(resources: Resource[]) { localStorage.setItem(STORAGE_KEY, JSON.stringify(resources)); }

export const api = {
  async resources(profileId?: string): Promise<Resource[]> {
    if (isNative()) return invokeTyped("resource_list", { profileId });
    return readDemo();
  },
  async create(draft: ResourceDraft): Promise<Resource> {
    if (isNative()) { const result = await invokeTyped("resource_create", { resource: draft }); await notifyWorkspaceChanged(draft.profileId); return result; }
    const resources = readDemo();
    const targets: ResourceTarget[] = draft.targets.map((target) => ({ id: target.id ?? crypto.randomUUID(), kind: target.kind, canonicalUri: target.canonicalUri, priority: target.priority, arguments: target.arguments, workingDirectory: target.workingDirectory || undefined, active: target.active }));
    const created: Resource = { ...draft, defaultAction: draft.defaultAction || "open", id: crypto.randomUUID(), createdUtc: new Date().toISOString(), updatedUtc: new Date().toISOString(), targets, tags: draft.tags || [], aliases: draft.aliases || [], favorite: false, launchCount: 0 };
    writeDemo([...resources, created]); return created;
  },
  async update(id: string, patch: ResourcePatch): Promise<Resource> {
    if (isNative()) { const result = await invokeTyped("resource_update", { id, patch }); await notifyWorkspaceChanged(patch.profileId); return result; }
    const resources: Resource[] = readDemo().map((r) => {
      if (r.id !== id) return r;
      const next = { ...r, ...patch, updatedUtc: new Date().toISOString() };
      const targets: ResourceTarget[] = (patch.targets ?? r.targets).map((target) => ({ id: target.id ?? crypto.randomUUID(), kind: target.kind, canonicalUri: target.canonicalUri, priority: target.priority, arguments: target.arguments, workingDirectory: target.workingDirectory || undefined, active: target.active }));
      return { ...next, targets };
    });
    writeDemo(resources); return resources.find((r) => r.id === id)!;
  },
  async remove(id: string, profileId?: string): Promise<void> {
    if (isNative()) { await invokeTyped("resource_delete", { id }); await notifyWorkspaceChanged(profileId); return; }
    writeDemo(readDemo().filter((r) => r.id !== id));
  },
  async launch(id: string, source: LaunchSource = "dashboard", profileId?: string): Promise<LaunchResult> {
    if (isNative()) { const result = await invokeTyped("resource_launch", { request: { requestId: crypto.randomUUID(), resourceId: id, source, profileId } }); if (result.status !== "failed") await notifyWorkspaceChanged(profileId); return result; }
    throw new Error("Native launch is available in the Windows desktop app. This browser preview does not open local resources.");
  },
  async search(query: string, profileId: string | undefined, fallbackResources: Resource[]): Promise<Resource[]> {
    if (isNative()) return invokeTyped("resource_search", { query, profileId, limit: 20 });
    const words = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
    const ranked = fallbackResources.map((resource) => {
      const fields = [resource.displayName, resource.description, resource.category, ...resource.aliases, ...resource.tags, ...resource.targets.map((target) => target.canonicalUri)].filter(Boolean).map((value) => value!.toLocaleLowerCase());
      const score = words.reduce((total, word) => total + Math.max(0, ...fields.map((field) => fuzzyScore(field, word))), 0);
      return { resource, score };
    }).filter(({ score }) => words.length === 0 || score > 0)
      .sort((a, b) => b.score - a.score || Number(b.resource.favorite) - Number(a.resource.favorite) || b.resource.launchCount - a.resource.launchCount);
    return ranked.slice(0, 20).map(({ resource }) => resource);
  },
  onQuickLaunchToggle(handler: () => void): Promise<() => void> {
    return isNative() ? listenTyped("quick-launch-toggle", handler) : Promise.resolve(() => undefined);
  },
  onQuickLaunchHotkeyStatus(handler: (status: QuickLaunchHotkeyStatus) => void): Promise<() => void> {
    return isNative() ? listenTyped("quick-launch-hotkey-status", handler) : Promise.resolve(() => undefined);
  },
  onWorkspaceStateChanged(handler: (payload: AppEventMap["workspace-state-changed"]) => void): Promise<() => void> {
    return isNative() ? listenTyped("workspace-state-changed", handler) : Promise.resolve(() => undefined);
  },
  async quickLaunchHotkeyStatus(): Promise<QuickLaunchHotkeyStatus> {
    if (isNative()) return invokeTyped("quick_launch_hotkey_status");
    return { registered: true, shortcut: "Alt+Space" };
  },
  async setQuickLaunchHotkey(profileId: string, shortcut: string): Promise<QuickLaunchHotkeyStatus> {
    if (isNative()) return invokeTyped("quick_launch_hotkey_set", { profileId, shortcut });
    const status = { registered: true, shortcut };
    localStorage.setItem(`swl-hotkey-${profileId}`, shortcut);
    return status;
  },
  async switchProfile(profileId: string): Promise<Profile> {
    if (isNative()) { const result = await invokeTyped("profile_switch", { profileId }); await notifyWorkspaceChanged(profileId); return result; }
    return (await api.profiles()).find((profile) => profile.id === profileId) || (await api.profiles())[0];
  },
  async hideCurrentWindow(): Promise<void> {
    if (!isNative()) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().hide();
  },
  async setDockVisibility(visible: boolean): Promise<void> {
    if (!isNative()) { localStorage.setItem("swl-dock-visible", String(visible)); return; }
    const { getAllWindows } = await import("@tauri-apps/api/window");
    const dock = (await getAllWindows()).find((item) => item.label === "dock");
    if (!dock) throw new Error("The Dock window is not available.");
    if (visible) await dock.show(); else await dock.hide();
  },
  async profiles(): Promise<Profile[]> {
    if (isNative()) return invokeTyped("profile_list");
    return [{ id: "default", name: "Personal", themeId: "system", settingsJson: "{}", createdUtc: now, updatedUtc: now }];
  },
  async layout(profileId: string, surface: "dashboard" | "dock" = "dashboard"): Promise<DashboardLayout | null> {
    if (isNative()) return invokeTyped("layout_get", { profileId, surface });
    try {
      const saved = localStorage.getItem(`swl-layout-${profileId}-${surface}`);
      return saved ? JSON.parse(saved) as DashboardLayout : null;
    } catch { return null; }
  },
  async saveLayout(layout: DashboardLayout): Promise<DashboardLayout> {
    if (isNative()) { const result = await invokeTyped("layout_save", { layout }); await notifyWorkspaceChanged(layout.profileId); return result; }
    localStorage.setItem(`swl-layout-${layout.profileId}-${layout.surface}`, JSON.stringify(layout));
    return layout;
  },
};

function fuzzyScore(value: string, term: string): number {
  if (value === term) return 1000;
  if (value.startsWith(term)) return 700 - (value.length - term.length);
  const substring = value.indexOf(term);
  if (substring >= 0) return 450 - substring;
  let cursor = 0; let gaps = 0;
  for (const char of term) {
    const next = value.indexOf(char, cursor);
    if (next < 0) return 0;
    gaps += next - cursor; cursor = next + 1;
  }
  return Math.max(1, 220 - gaps - (value.length - term.length));
}
