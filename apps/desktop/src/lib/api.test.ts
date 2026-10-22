import { afterEach, describe, expect, it, vi } from "vitest";
import type { DashboardLayout, ResourceRelocationPreview, ResourceRelationship, WorkspaceSnapshot } from "@sw/contracts";

const { emitTyped, invokeTyped, listenTyped } = vi.hoisted(() => ({ emitTyped: vi.fn(), invokeTyped: vi.fn(), listenTyped: vi.fn() }));
vi.mock("@sw/contracts", () => ({ emitTyped, invokeTyped, listenTyped }));
import { api } from "./api";
import type { Resource } from "./contracts";

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

describe("smart collections API", () => {
  const collection = { id: "c1", profileId: "p1", name: "Planning files", filters: { resourceType: "file" as const, tag: "planning", favoriteOnly: true } };
  it("persists collection definitions through typed native commands", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    invokeTyped.mockResolvedValueOnce([collection]).mockResolvedValueOnce(collection).mockResolvedValueOnce(undefined);
    expect(await api.smartCollections("p1")).toEqual([collection]);
    expect(invokeTyped).toHaveBeenNthCalledWith(1, "smart_collection_list", { profileId: "p1" });
    expect(await api.saveSmartCollection(collection)).toEqual(collection);
    expect(invokeTyped).toHaveBeenNthCalledWith(2, "smart_collection_save", { collection });
    await api.deleteSmartCollection("p1", "c1");
    expect(invokeTyped).toHaveBeenNthCalledWith(3, "smart_collection_delete", { profileId: "p1", id: "c1" });
    expect(emitTyped).toHaveBeenNthCalledWith(1, "workspace-state-changed", { profileId: "p1" });
    expect(emitTyped).toHaveBeenNthCalledWith(2, "workspace-state-changed", { profileId: "p1" });
  });
  it("keeps browser-preview collections isolated by profile and supports reload reads", async () => {
    await api.saveSmartCollection(collection);
    expect(await api.smartCollections("p1")).toEqual([collection]);
    expect(await api.smartCollections("p2")).toEqual([]);
    const updated = { ...collection, name: "Updated planning" };
    await api.saveSmartCollection(updated);
    expect(await api.smartCollections("p1")).toEqual([updated]);
    await api.deleteSmartCollection("p1", "c1");
    expect(await api.smartCollections("p1")).toEqual([]);
    expect(localStorage.getItem("swl-smart-collections-p2")).toBeNull();
  });
});

describe("dynamic shortcut API", () => {
  const shortcut = { id: "ds1", profileId: "p1", name: "Weekly report", resourceId: "r1", targetId: "t1" };
  it("uses typed profile-scoped list/save/delete commands", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    invokeTyped.mockResolvedValueOnce([shortcut]).mockResolvedValueOnce(shortcut).mockResolvedValueOnce(undefined);
    expect(await api.dynamicShortcuts("p1")).toEqual([shortcut]);
    expect(invokeTyped).toHaveBeenNthCalledWith(1, "dynamic_shortcut_list", { profileId: "p1" });
    expect(await api.saveDynamicShortcut(shortcut)).toEqual(shortcut);
    expect(invokeTyped).toHaveBeenNthCalledWith(2, "dynamic_shortcut_save", { shortcut });
    await api.deleteDynamicShortcut("p1", "ds1");
    expect(invokeTyped).toHaveBeenNthCalledWith(3, "dynamic_shortcut_delete", { profileId: "p1", id: "ds1" });
    expect(emitTyped).toHaveBeenNthCalledWith(1, "workspace-state-changed", { profileId: "p1" });
    expect(emitTyped).toHaveBeenNthCalledWith(2, "workspace-state-changed", { profileId: "p1" });
  });
  it("keeps browser preview shortcuts isolated and persistent per profile", async () => {
    await api.saveDynamicShortcut(shortcut);
    expect(await api.dynamicShortcuts("p1")).toEqual([shortcut]);
    expect(await api.dynamicShortcuts("p2")).toEqual([]);
    await api.deleteDynamicShortcut("p1", "ds1");
    expect(await api.dynamicShortcuts("p1")).toEqual([]);
    expect(localStorage.getItem("swl-dynamic-shortcuts-p2")).toBeNull();
  });
  it("launches only the referenced resource with the selected target through the normal command", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    const result = { requestId: "q1", status: "handed_off", launchedAtUtc: "2026-01-01T00:00:00Z" };
    invokeTyped.mockResolvedValue(result);
    expect(await api.launch(shortcut.resourceId, "quick_launch", shortcut.profileId, shortcut.targetId)).toEqual(result);
    expect(invokeTyped).toHaveBeenCalledWith("resource_launch", { request: expect.objectContaining({ resourceId: "r1", targetId: "t1", profileId: "p1", source: "quick_launch" }) });
  });
});

describe("workspace sequence API", () => {
  const sequence = { id: "s1", profileId: "p1", name: "Morning setup", steps: [{ id: "st1", resourceId: "r1", targetId: "t1" }, { id: "st2", resourceId: "r2" }] };
  it("uses typed native list/save/delete/run commands", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    const result = { sequenceId: "s1", status: "completed" as const, totalSteps: 2, completedSteps: 2, steps: [] };
    invokeTyped.mockResolvedValueOnce([sequence]).mockResolvedValueOnce(sequence).mockResolvedValueOnce(undefined).mockResolvedValueOnce(result);
    expect(await api.workspaceSequences("p1")).toEqual([sequence]);
    expect(invokeTyped).toHaveBeenNthCalledWith(1, "workspace_sequence_list", { profileId: "p1" });
    expect(await api.saveWorkspaceSequence(sequence)).toEqual(sequence);
    expect(invokeTyped).toHaveBeenNthCalledWith(2, "workspace_sequence_save", { sequence });
    await api.deleteWorkspaceSequence("p1", "s1");
    expect(invokeTyped).toHaveBeenNthCalledWith(3, "workspace_sequence_delete", { profileId: "p1", id: "s1" });
    expect(await api.runWorkspaceSequence("p1", "s1", "run-1")).toEqual(result);
    expect(invokeTyped).toHaveBeenNthCalledWith(4, "workspace_sequence_run", { profileId: "p1", id: "s1", requestId: "run-1" });
    expect(emitTyped).toHaveBeenCalledTimes(2);
  });
  it("stores sequences by profile in the browser preview", async () => {
    expect(await api.workspaceSequences("p1")).toEqual([]);
    await api.saveWorkspaceSequence(sequence);
    expect(await api.workspaceSequences("p1")).toEqual([sequence]);
    expect(await api.workspaceSequences("p2")).toEqual([]);
    await api.deleteWorkspaceSequence("p1", "s1");
    expect(await api.workspaceSequences("p1")).toEqual([]);
  });
  it("runs in order and stops at the first failed handoff in preview mode", async () => {
    await api.saveWorkspaceSequence(sequence);
    const launch = vi.spyOn(api, "launch").mockResolvedValueOnce({ requestId: "r1", status: "handed_off", launchedAtUtc: "now" }).mockResolvedValueOnce({ requestId: "r2", status: "failed", launchedAtUtc: "now", message: "Unavailable" });
    const result = await api.runWorkspaceSequence("p1", "s1", "run-1");
    expect(result).toMatchObject({ status: "failed", totalSteps: 2, completedSteps: 1, stoppedAtStepId: "st2" });
    expect(launch).toHaveBeenNthCalledWith(1, "r1", "sequence", "p1", "t1");
    expect(launch).toHaveBeenNthCalledWith(2, "r2", "sequence", "p1", undefined);
    expect(launch).toHaveBeenCalledTimes(2);
    launch.mockRestore();
  });
});

describe("resource relationship and workspace snapshot API", () => {
  const relationship: ResourceRelationship = { id: "edge1", profileId: "p1", sourceResourceId: "r1", targetResourceId: "r2", label: "supports" };
  const snapshot: WorkspaceSnapshot = { id: "snap1", profileId: "p1", name: "Planning", createdUtc: "2026-01-01T00:00:00Z", resourceIds: ["r1", "r2"], dashboard: { profileId: "p1", surface: "dashboard", version: 1, cards: [] }, dock: { profileId: "p1", surface: "dock", version: 1, cards: [] } };

  it("routes all relationship and snapshot operations through typed native commands", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    const restored = { snapshotId: "snap1", restoredLayouts: 2, missingResourceIds: [] };
    invokeTyped.mockResolvedValueOnce([relationship]).mockResolvedValueOnce(relationship).mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce([snapshot]).mockResolvedValueOnce(snapshot).mockResolvedValueOnce(undefined).mockResolvedValueOnce(restored);
    expect(await api.resourceRelationships("p1")).toEqual([relationship]);
    expect(await api.saveResourceRelationship(relationship)).toEqual(relationship);
    await api.deleteResourceRelationship("p1", "edge1");
    expect(await api.workspaceSnapshots("p1")).toEqual([snapshot]);
    expect(await api.createWorkspaceSnapshot("p1", "Planning")).toEqual(snapshot);
    await api.deleteWorkspaceSnapshot("p1", "snap1");
    expect(await api.restoreWorkspaceSnapshot("p1", "snap1")).toEqual(restored);
    expect(invokeTyped.mock.calls.map(([command]) => command)).toEqual([
      "resource_relationship_list", "resource_relationship_save", "resource_relationship_delete",
      "workspace_snapshot_list", "workspace_snapshot_create", "workspace_snapshot_delete", "workspace_snapshot_restore",
    ]);
    expect(emitTyped).toHaveBeenCalledTimes(5);
  });

  it("keeps browser-preview graph records profile-local and rejects reversed duplicate edges", async () => {
    await api.saveResourceRelationship(relationship);
    expect(await api.resourceRelationships("p1")).toEqual([relationship]);
    expect(await api.resourceRelationships("p2")).toEqual([]);
    await expect(api.saveResourceRelationship({ ...relationship, id: "edge2", sourceResourceId: "r2", targetResourceId: "r1" })).rejects.toThrow("already linked");
    await api.deleteResourceRelationship("p1", "edge1");
    expect(await api.resourceRelationships("p1")).toEqual([]);
  });

  it("restores only currently present resources and reports stale snapshot references", async () => {
    const valid = { ...snapshot.dashboard.cards[0], resourceId: "a1", x: 0, y: 0, w: 4, h: 1, type: "icon" as const };
    const stale = { ...valid, resourceId: "removed-resource", y: 1 };
    const savedSnapshot = { ...snapshot, resourceIds: ["a1", "removed-resource"], dashboard: { ...snapshot.dashboard, cards: [valid, stale] } };
    localStorage.setItem("swl-workspace-snapshots-p1", JSON.stringify([savedSnapshot]));
    const result = await api.restoreWorkspaceSnapshot("p1", "snap1");
    expect(result).toEqual({ snapshotId: "snap1", restoredLayouts: 2, missingResourceIds: ["removed-resource"] });
    expect(await api.layout("p1")).toEqual({ ...savedSnapshot.dashboard, cards: [valid] });
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
    const filter = { fromUtc: "2025-01-01T00:00:00Z", source: "dock" as const };
    expect(await api.analyticsSummary("p1", filter)).toEqual(summary);
    expect(await api.analyticsEvents({ profileId: "p1", ...filter, limit: 42, offset: 0 })).toEqual(events);
    expect(invokeTyped.mock.calls).toEqual([
      ["resource_inspect", { resourceId: "r1", forceRefresh: true }],
      ["analytics_summary", { profileId: "p1", ...filter }],
      ["analytics_query", { profileId: "p1", ...filter, limit: 42, offset: 0 }],
    ]);
  });

  it("keeps browser preview clearly separate from native records", async () => {
    expect(api.isNativeApp()).toBe(false);
    expect(await api.inspect("r1")).toEqual([]);
    expect(await api.analyticsSummary("p1")).toEqual({ totalLaunches: 0, bySource: {}, byStatus: {} });
    expect(await api.analyticsEvents({ profileId: "p1", limit: 42, offset: 0 })).toEqual([]);
    expect(invokeTyped).not.toHaveBeenCalled();
  });

  it("uses typed history settings, purge, and CSV export commands", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    const settings = { profileId: "p1", recordingEnabled: false, retentionDays: 90 as const, excludedResourceIds: ["r2"] };
    const exported = { path: "C:\\Exports\\usage.csv", eventCount: 4 };
    invokeTyped.mockResolvedValueOnce(settings).mockResolvedValueOnce(settings).mockResolvedValueOnce(4).mockResolvedValueOnce(exported);
    expect(await api.usageSettings("p1")).toEqual(settings);
    expect(await api.saveUsageSettings(settings)).toEqual(settings);
    expect(await api.purgeUsageHistory("p1")).toBe(4);
    expect(await api.exportUsageCsv({ profileId: "p1", source: "dock", limit: 20, offset: 0 }, exported.path)).toEqual(exported);
    expect(invokeTyped.mock.calls).toEqual([
      ["usage_settings_get", { profileId: "p1" }],
      ["usage_settings_set", { settings }],
      ["usage_history_purge", { profileId: "p1" }],
      ["usage_export_csv", { query: { profileId: "p1", source: "dock", limit: 20, offset: 0 }, outputPath: exported.path }],
    ]);
  });
});

describe("profile archive API", () => {
  it("uses the frozen typed preview, export and commit boundaries", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    const preview = { profileId: "p", profileName: "Work", targetProfileId: "p", packageSha256: "sha256", previewToken: "preview-token", resourceCount: 3, layoutCount: 1, historyEventCount: 2, smartCollectionCount: 1, dynamicShortcutCount: 2, profileIdCollision: true, profileNameCollision: true, resourceIdCollisions: ["r1"], targetIdCollisions: [], smartCollectionIdCollisions: [], smartCollectionNameCollisions: [], dynamicShortcutIdCollisions: [], dynamicShortcutNameCollisions: [], resourceConflicts: [], assetCount: 0 };
    const exported = { path: "C:\\Work.swprofile", profileName: "Work", resourceCount: 3, historyEventCount: 2 };
    const imported = { profileId: "p2", profileName: "Work (2)", importedResources: 3, reusedResources: 0, importedLayouts: 1, importedHistoryEvents: 2 };
    invokeTyped.mockResolvedValueOnce(exported).mockResolvedValueOnce(preview).mockResolvedValueOnce(preview).mockResolvedValueOnce(imported);
    expect(await api.exportProfile("p", exported.path, true)).toEqual(exported);
    expect(await api.previewProfileImport(exported.path)).toEqual(preview);
    expect(await api.previewProfileImport(exported.path, "p")).toEqual(preview);
    expect(await api.commitProfileImport(exported.path, "replace", "p", preview.packageSha256, preview.previewToken)).toEqual(imported);
    expect(emitTyped).toHaveBeenCalledWith("workspace-state-changed", { profileId: "p2" });
    expect(invokeTyped.mock.calls).toEqual([
      ["profile_export", { profileId: "p", outputPath: exported.path, includeHistory: true }],
      ["profile_import_preview", { packagePath: exported.path }],
      ["profile_import_preview", { packagePath: exported.path, targetProfileId: "p" }],
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

describe("duplicate and relocation API", () => {
  it("duplicates a resource with fresh IDs and no copied usage history", async () => {
    const source = (await api.resources("p1")).find((resource) => resource.id === "a1")!;
    const copy = await api.duplicateResource({ profileId: "p1", resourceId: source.id });
    expect(copy.displayName).toBe(`Copy of ${source.displayName}`);
    expect(copy.id).not.toBe(source.id);
    expect(copy.targets).toHaveLength(source.targets.length);
    expect(copy.targets[0].id).not.toBe(source.targets[0].id);
    expect(copy.targets[0].canonicalUri).toBe(source.targets[0].canonicalUri);
    expect(copy.launchCount).toBe(0);
    expect(copy.lastLaunchedUtc).toBeNull();
    expect((await api.resources("p1")).some((resource) => resource.id === copy.id)).toBe(true);
  });

  it("uses native typed commands for duplicate, relocation preview, and preview-bound commit", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    const duplicate = { id: "copy", displayName: "Copy", targets: [], favorite: false, launchCount: 0 } as unknown as Resource;
    const preview: ResourceRelocationPreview = { profileId: "p1", sourcePrefix: "C:\\Old", destinationPrefix: "D:\\New", previewToken: "bound", changes: [] };
    const result = { updatedTargetCount: 1, updatedResourceIds: ["r1"] };
    invokeTyped.mockResolvedValueOnce(duplicate).mockResolvedValueOnce(preview).mockResolvedValueOnce(result);
    expect(await api.duplicateResource({ profileId: "p1", resourceId: "r1" })).toBe(duplicate);
    expect(await api.previewResourceRelocation({ profileId: "p1", sourcePrefix: preview.sourcePrefix, destinationPrefix: preview.destinationPrefix })).toBe(preview);
    expect(await api.commitResourceRelocation(preview)).toBe(result);
    expect(invokeTyped.mock.calls).toEqual([
      ["resource_duplicate", { request: { profileId: "p1", resourceId: "r1" } }],
      ["resource_relocation_preview", { request: { profileId: "p1", sourcePrefix: "C:\\Old", destinationPrefix: "D:\\New" } }],
      ["resource_relocation_commit", { preview }],
    ]);
  });

  it("previews only segment-boundary file and folder targets, then commits after confirmation", async () => {
    const preview = await api.previewResourceRelocation({ profileId: "p1", sourcePrefix: "C:\\Workspace", destinationPrefix: "D:\\Archive" });
    expect(preview.changes.map(({ fromUri, toUri }) => [fromUri, toUri])).toEqual([
      ["C:\\Workspace\\Design review", "D:\\Archive\\Design review"],
      ["C:\\Workspace\\Q3 Planning.xlsx", "D:\\Archive\\Q3 Planning.xlsx"],
    ]);
    expect((await api.resources("p1")).find((resource) => resource.id === "a1")?.targets[0].canonicalUri).toBe("C:\\Workspace\\Design review");
    const result = await api.commitResourceRelocation(preview);
    expect(result).toEqual({ updatedTargetCount: 2, updatedResourceIds: ["a1", "a2"] });
    expect((await api.resources("p1")).find((resource) => resource.id === "a1")?.targets[0].canonicalUri).toBe("D:\\Archive\\Design review");
  });

  it("does not match a sibling path that only shares the prefix text", async () => {
    const items = await api.resources("p1");
    const template = items.find((resource) => resource.id === "a1")!;
    const sibling = { ...template, id: "outside-prefix", displayName: "Sibling folder", targets: [{ ...template.targets[0], id: "outside-target", canonicalUri: "C:\\Workspace backup\\file.txt" }] };
    localStorage.setItem("swl-ui-demo-resources-v1", JSON.stringify([...items, sibling]));
    const preview = await api.previewResourceRelocation({ profileId: "p1", sourcePrefix: "C:\\Workspace", destinationPrefix: "D:\\Archive" });
    expect(preview.changes.some((change) => change.resourceId === sibling.id)).toBe(false);
    await expect(api.previewResourceRelocation({ profileId: "p1", sourcePrefix: "C:\\Workspace\\..\\private", destinationPrefix: "D:\\Archive" })).rejects.toThrow("cannot contain . or ..");
  });

  it("rejects changed targets and malformed prefixes without applying a partial relocation", async () => {
    const preview = await api.previewResourceRelocation({ profileId: "p1", sourcePrefix: "C:\\Workspace", destinationPrefix: "D:\\Archive" });
    const resource = (await api.resources("p1")).find((item) => item.id === "a1")!;
    await api.update(resource.id, { targets: resource.targets.map((target) => ({ ...target, canonicalUri: "C:\\Moved\\Design review" })) });
    await expect(api.commitResourceRelocation(preview)).rejects.toThrow("Resources changed after the preview");
    expect((await api.resources("p1")).find((item) => item.id === "a2")?.targets[0].canonicalUri).toBe("C:\\Workspace\\Q3 Planning.xlsx");
    await expect(api.previewResourceRelocation({ profileId: "p1", sourcePrefix: "https://example.com", destinationPrefix: "D:\\Archive" })).rejects.toThrow("not a URL");
  });
});
