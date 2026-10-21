import { emitTyped, invokeTyped, listenTyped } from "@sw/contracts";
import type { AnalyticsSummary, AppEventMap, DashboardLayout, DynamicShortcut, LaunchEvent, LaunchSource, ProfileExportResult, ProfileImportPreview, ProfileImportResult, ProfileImportStrategy, ProfileThemeInput, QuickLaunchHotkeyStatus, ResourceMetadata, ResourcePatch, SmartCollection, UsageExportResult, UsageFilter, UsageQuery, UsageSettings, WorkspaceSequence, WorkspaceSequenceRunResult } from "@sw/contracts";
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
  async setProfileTheme(profileId: string, theme: ProfileThemeInput): Promise<Profile> {
    if (isNative()) {
      const updated = await invokeTyped("profile_theme_set", { profileId, theme });
      await notifyWorkspaceChanged(profileId);
      return updated;
    }
    const profile = (await api.profiles()).find((item) => item.id === profileId);
    if (!profile) throw new Error("Workspace profile was not found.");
    let settings: Record<string, unknown> = {};
    try { settings = JSON.parse(profile.settingsJson || "{}") as Record<string, unknown>; } catch { /* replace malformed demo settings */ }
    const nextSettings: Record<string, unknown> = { ...settings, accent: theme.accent };
    if (theme.density) nextSettings.density = theme.density;
    else delete nextSettings.density;
    const settingsJson = JSON.stringify(nextSettings);
    const updated = { ...profile, themeId: theme.mode, settingsJson };
    localStorage.setItem(`swl-profile-theme-${profileId}`, JSON.stringify(updated));
    return updated;
  },
  async exportProfile(profileId: string, outputPath: string, includeHistory = false): Promise<ProfileExportResult> {
    if (!isNative()) throw new Error("Profile export is available in the Windows desktop app.");
    return invokeTyped("profile_export", { profileId, outputPath, includeHistory });
  },
  async previewProfileImport(packagePath: string, targetProfileId?: string): Promise<ProfileImportPreview> {
    if (!isNative()) throw new Error("Profile import is available in the Windows desktop app.");
    return invokeTyped("profile_import_preview", { packagePath, ...(targetProfileId ? { targetProfileId } : {}) });
  },
  async commitProfileImport(packagePath: string, strategy: ProfileImportStrategy, targetProfileId: string | undefined, packageSha256: string, previewToken: string): Promise<ProfileImportResult> {
    if (!isNative()) throw new Error("Profile import is available in the Windows desktop app.");
    const result = await invokeTyped("profile_import_commit", { packagePath, strategy, targetProfileId, packageSha256, previewToken });
    await notifyWorkspaceChanged(result.profileId);
    return result;
  },
  isNativeApp(): boolean { return isNative(); },
  async inspect(resourceId: string, forceRefresh = false): Promise<ResourceMetadata[]> {
    if (isNative()) return invokeTyped("resource_inspect", { resourceId, forceRefresh });
    return [];
  },
  async analyticsSummary(profileId: string, filter: UsageFilter = {}): Promise<AnalyticsSummary> {
    if (isNative()) return invokeTyped("analytics_summary", { profileId, ...filter });
    return { totalLaunches: 0, bySource: {}, byStatus: {} };
  },
  async analyticsEvents(query: UsageQuery): Promise<LaunchEvent[]> {
    if (isNative()) return invokeTyped("analytics_query", query);
    return [];
  },
  async usageSettings(profileId: string): Promise<UsageSettings> {
    if (isNative()) return invokeTyped("usage_settings_get", { profileId });
    try {
      const saved = localStorage.getItem(`swl-usage-settings-${profileId}`);
      if (saved) return JSON.parse(saved) as UsageSettings;
    } catch { /* use safe defaults for the browser preview */ }
    return { profileId, recordingEnabled: true, retentionDays: null, excludedResourceIds: [] };
  },
  async saveUsageSettings(settings: UsageSettings): Promise<UsageSettings> {
    if (isNative()) return invokeTyped("usage_settings_set", { settings });
    localStorage.setItem(`swl-usage-settings-${settings.profileId}`, JSON.stringify(settings));
    return settings;
  },
  async purgeUsageHistory(profileId: string): Promise<number> {
    if (isNative()) return invokeTyped("usage_history_purge", { profileId });
    return 0;
  },
  async exportUsageCsv(query: UsageQuery, outputPath: string): Promise<UsageExportResult> {
    if (!isNative()) throw new Error("Usage export is available in the Windows desktop app.");
    return invokeTyped("usage_export_csv", { query, outputPath });
  },
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
  async launch(id: string, source: LaunchSource = "dashboard", profileId?: string, targetId?: string): Promise<LaunchResult> {
    if (isNative()) { const result = await invokeTyped("resource_launch", { request: { requestId: crypto.randomUUID(), resourceId: id, source, profileId, ...(targetId ? { targetId } : {}) } }); if (result.status !== "failed") await notifyWorkspaceChanged(profileId); return result; }
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
    const profile: Profile = { id: "default", name: "Personal", themeId: "system", settingsJson: "{}", createdUtc: now, updatedUtc: now };
    try { const saved = localStorage.getItem("swl-profile-theme-default"); return [saved ? { ...profile, ...JSON.parse(saved) as Profile } : profile]; } catch { return [profile]; }
  },
  async smartCollections(profileId: string): Promise<SmartCollection[]> {
    if (isNative()) return invokeTyped("smart_collection_list", { profileId });
    try { return JSON.parse(localStorage.getItem(`swl-smart-collections-${profileId}`) || "[]") as SmartCollection[]; } catch { return []; }
  },
  async saveSmartCollection(collection: SmartCollection): Promise<SmartCollection> {
    if (isNative()) {
      const saved = await invokeTyped("smart_collection_save", { collection });
      await notifyWorkspaceChanged(collection.profileId);
      return saved;
    }
    const current = await api.smartCollections(collection.profileId);
    const index = current.findIndex((item) => item.id === collection.id);
    const saved = { ...collection };
    const next = index < 0 ? [...current, saved] : current.map((item, itemIndex) => itemIndex === index ? saved : item);
    localStorage.setItem(`swl-smart-collections-${collection.profileId}`, JSON.stringify(next));
    return saved;
  },
  async deleteSmartCollection(profileId: string, id: string): Promise<void> {
    if (isNative()) { await invokeTyped("smart_collection_delete", { profileId, id }); await notifyWorkspaceChanged(profileId); return; }
    const next = (await api.smartCollections(profileId)).filter((item) => item.id !== id);
    localStorage.setItem(`swl-smart-collections-${profileId}`, JSON.stringify(next));
  },
  async dynamicShortcuts(profileId: string): Promise<DynamicShortcut[]> {
    if (isNative()) return invokeTyped("dynamic_shortcut_list", { profileId });
    try { return JSON.parse(localStorage.getItem(`swl-dynamic-shortcuts-${profileId}`) || "[]") as DynamicShortcut[]; } catch { return []; }
  },
  async saveDynamicShortcut(shortcut: DynamicShortcut): Promise<DynamicShortcut> {
    if (isNative()) { const saved = await invokeTyped("dynamic_shortcut_save", { shortcut }); await notifyWorkspaceChanged(shortcut.profileId); return saved; }
    const current = await api.dynamicShortcuts(shortcut.profileId);
    const next = current.some((item) => item.id === shortcut.id) ? current.map((item) => item.id === shortcut.id ? shortcut : item) : [...current, shortcut];
    localStorage.setItem(`swl-dynamic-shortcuts-${shortcut.profileId}`, JSON.stringify(next));
    return shortcut;
  },
  async deleteDynamicShortcut(profileId: string, id: string): Promise<void> {
    if (isNative()) { await invokeTyped("dynamic_shortcut_delete", { profileId, id }); await notifyWorkspaceChanged(profileId); return; }
    localStorage.setItem(`swl-dynamic-shortcuts-${profileId}`, JSON.stringify((await api.dynamicShortcuts(profileId)).filter((item) => item.id !== id)));
  },
  async workspaceSequences(profileId: string): Promise<WorkspaceSequence[]> {
    if (isNative()) return invokeTyped("workspace_sequence_list", { profileId });
    try { return JSON.parse(localStorage.getItem(`swl-workspace-sequences-${profileId}`) || "[]") as WorkspaceSequence[]; } catch { return []; }
  },
  async saveWorkspaceSequence(sequence: WorkspaceSequence): Promise<WorkspaceSequence> {
    if (isNative()) { const saved = await invokeTyped("workspace_sequence_save", { sequence }); await notifyWorkspaceChanged(sequence.profileId); return saved; }
    const current = await api.workspaceSequences(sequence.profileId);
    const next = current.some((item) => item.id === sequence.id) ? current.map((item) => item.id === sequence.id ? sequence : item) : [...current, sequence];
    localStorage.setItem(`swl-workspace-sequences-${sequence.profileId}`, JSON.stringify(next));
    return sequence;
  },
  async deleteWorkspaceSequence(profileId: string, id: string): Promise<void> {
    if (isNative()) { await invokeTyped("workspace_sequence_delete", { profileId, id }); await notifyWorkspaceChanged(profileId); return; }
    localStorage.setItem(`swl-workspace-sequences-${profileId}`, JSON.stringify((await api.workspaceSequences(profileId)).filter((item) => item.id !== id)));
  },
  async runWorkspaceSequence(profileId: string, id: string, requestId: string = crypto.randomUUID()): Promise<WorkspaceSequenceRunResult> {
    if (isNative()) return invokeTyped("workspace_sequence_run", { profileId, id, requestId });
    const sequence = (await api.workspaceSequences(profileId)).find((item) => item.id === id);
    if (!sequence) throw new Error("Workspace sequence was not found.");
    const steps: WorkspaceSequenceRunResult["steps"] = [];
    for (const step of sequence.steps) {
      let result: LaunchResult;
      try { result = await api.launch(step.resourceId, "sequence", profileId, step.targetId); }
      catch (error) { result = { requestId: crypto.randomUUID(), status: "failed", launchedAtUtc: new Date().toISOString(), errorCode: "launch_failed", message: error instanceof Error ? error.message : "Resource launch failed." }; }
      steps.push({ stepId: step.id, resourceId: step.resourceId, ...(step.targetId ? { targetId: step.targetId } : {}), result });
      if (result.status === "failed") break;
    }
    const completedSteps = steps.filter((step) => step.result.status !== "failed").length;
    const stoppedAtStepId = steps.at(-1)?.result.status === "failed" ? steps.at(-1)!.stepId : undefined;
    return { sequenceId: id, status: stoppedAtStepId ? "failed" : "completed", totalSteps: sequence.steps.length, completedSteps, ...(stoppedAtStepId ? { stoppedAtStepId } : {}), steps };
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
