/** Stable JSON boundary shared by the React client and Tauri commands. */
export type ResourceType = "file" | "folder" | "app" | "shortcut" | "url";
export type TargetKind = ResourceType;
export type LaunchStatus = "requested" | "handed_off" | "failed";
export type LaunchSource = "dashboard" | "quick_launch" | "dock" | "explorer" | "sequence" | "other";

export interface ResourceTarget {
  id: string;
  kind: TargetKind;
  canonicalUri: string;
  priority: number;
  arguments: string[];
  workingDirectory?: string | null;
  active: boolean;
}

export interface Resource {
  id: string;
  resourceType: ResourceType;
  displayName: string;
  description?: string | null;
  icon?: string | null;
  defaultAction: string;
  createdUtc: string;
  updatedUtc: string;
  targets: ResourceTarget[];
  tags: string[];
  aliases: string[];
  category?: string | null;
  favorite: boolean;
  launchCount: number;
  lastLaunchedUtc?: string | null;
}

export interface ResourceInput {
  profileId?: string;
  resourceType: ResourceType;
  displayName: string;
  description?: string | null;
  icon?: string | null;
  defaultAction?: string;
  targets: Array<Omit<ResourceTarget, "id"> & { id?: string }>;
  tags?: string[];
  aliases?: string[];
  category?: string;
}

export type ResourcePatch = Partial<Omit<ResourceInput, "targets">> & {
  profileId?: string;
  favorite?: boolean;
  targets?: ResourceInput["targets"];
};

export interface Profile {
  id: string;
  name: string;
  themeId: string;
  settingsJson: string;
  createdUtc: string;
  updatedUtc: string;
}

export interface ProfileThemeInput {
  mode: "light" | "dark" | "system";
  /** Custom accent color in #RRGGBB form. */
  accent: string;
  density?: "compact" | "balanced" | "comfortable";
}

/** Safe, bounded filter criteria; all populated fields are combined with AND. */
export interface SmartCollectionFilters {
  resourceType?: ResourceType;
  category?: string;
  tag?: string;
  favoriteOnly?: boolean;
  text?: string;
}

export interface SmartCollection {
  id: string;
  profileId: string;
  name: string;
  filters: SmartCollectionFilters;
}

/** A named profile-local reference to an existing resource and optional target. */
export interface DynamicShortcut {
  id: string;
  profileId: string;
  name: string;
  resourceId: string;
  targetId?: string;
}

/** A profile-local ordered sequence of existing resource references; steps never contain executable commands. */
export interface WorkspaceSequenceStep {
  id: string;
  resourceId: string;
  targetId?: string;
}

export interface WorkspaceSequence {
  id: string;
  profileId: string;
  name: string;
  steps: WorkspaceSequenceStep[];
}

export interface WorkspaceSequenceStepResult {
  stepId: string;
  resourceId: string;
  targetId?: string;
  result: LaunchResult;
}

export interface WorkspaceSequenceRunResult {
  sequenceId: string;
  status: "completed" | "failed" | "in_progress";
  totalSteps: number;
  completedSteps: number;
  stoppedAtStepId?: string;
  steps: WorkspaceSequenceStepResult[];
}

/** A profile-local undirected edge between two resources in the same profile. */
export interface ResourceRelationship {
  id: string;
  profileId: string;
  sourceResourceId: string;
  targetResourceId: string;
  label: string;
}

/** Non-destructive checkpoint of a profile's dashboard and dock arrangements. */
export interface WorkspaceSnapshot {
  id: string;
  profileId: string;
  name: string;
  createdUtc: string;
  resourceIds: string[];
  dashboard: DashboardLayout;
  dock: DashboardLayout;
}

export interface WorkspaceSnapshotRestoreResult {
  snapshotId: string;
  restoredLayouts: number;
  missingResourceIds: string[];
}

export interface LaunchRequest {
  requestId: string;
  resourceId: string;
  profileId?: string;
  targetId?: string;
  source: LaunchSource;
}

/** Shell handoff success does not mean that the target application loaded. */
export interface LaunchResult {
  requestId: string;
  status: LaunchStatus;
  launchedAtUtc: string;
  errorCode?: string | null;
  message?: string | null;
}

export interface LaunchEvent extends LaunchResult {
  resourceId: string;
  targetId?: string | null;
  profileId?: string | null;
  source: LaunchSource;
  durationMs?: number | null;
}

/** Metadata comes only from inspecting the selected target path; it does not observe other apps. */
export interface ResourceMetadata {
  resourceId: string;
  targetId: string;
  inspectedUtc: string;
  fileSizeBytes?: number;
  createdUtc?: string;
  modifiedUtc?: string;
  availability: "available" | "missing" | "inaccessible" | "unsupported";
  provenance: "local-filesystem" | "unsupported";
  cacheStatus: "fresh" | "cached" | "error";
  errorCode?: string;
}

/** Aggregates only launch events recorded by this app on this device. */
export interface AnalyticsSummary {
  totalLaunches: number;
  bySource: Record<string, number>;
  byStatus: Record<string, number>;
  firstEventUtc?: string;
  lastEventUtc?: string;
}

/** Local, profile-scoped history controls. A null retention window keeps recorded events until purged. */
export interface UsageSettings {
  profileId: string;
  recordingEnabled: boolean;
  retentionDays: 30 | 90 | 365 | null;
  excludedResourceIds: string[];
}

/** UTC bounds are inclusive; all populated filters are combined with AND. */
export interface UsageFilter {
  fromUtc?: string;
  toUtc?: string;
  source?: LaunchSource;
  status?: LaunchStatus;
  resourceId?: string;
}

export interface UsageQuery extends UsageFilter {
  profileId: string;
  limit?: number;
  offset?: number;
}

export interface UsageExportResult {
  path: string;
  eventCount: number;
}

export interface QuickLaunchHotkeyStatus {
  /** Whether the requested shortcut is currently registered. */
  registered: boolean;
  /** Requested or configured shortcut. */
  shortcut: string;
  /** Previously active shortcut when a replacement failed. */
  activeShortcut?: string;
  errorCode?: string;
}

export interface SearchRequest {
  query: string;
  profileId?: string;
  limit?: number;
}

export interface LayoutCard {
  resourceId: string;
  x: number;
  y: number;
  w: number;
  h: number;
  type: "icon" | "compact" | "info" | "group";
}

export interface DashboardLayout {
  profileId: string;
  surface: "dashboard" | "dock";
  version: number;
  cards: LayoutCard[];
}

export type ProfileImportStrategy = "duplicate" | "merge" | "replace";

export interface ProfileImportPreview {
  packageSha256: string;
  previewToken: string;
  profileId: string;
  profileName: string;
  targetProfileId?: string;
  resourceCount: number;
  layoutCount: number;
  historyEventCount: number;
  smartCollectionCount: number;
  dynamicShortcutCount: number;
  workspaceSequenceCount: number;
  profileIdCollision: boolean;
  profileNameCollision: boolean;
  resourceIdCollisions: string[];
  targetIdCollisions: string[];
  resourceConflicts: ProfileResourceConflictPreview[];
  smartCollectionIdCollisions: string[];
  smartCollectionNameCollisions: string[];
  dynamicShortcutIdCollisions: string[];
  dynamicShortcutNameCollisions: string[];
  workspaceSequenceIdCollisions: string[];
  workspaceSequenceNameCollisions: string[];
  assetCount: number;
}

export interface ProfileTargetSnapshot {
  id: string;
  kind: TargetKind;
  value: string;
  priority: number;
  arguments: string[];
  workingDirectory?: string | null;
  active: boolean;
}

export interface ProfileResourceSnapshot {
  id: string;
  name: string;
  resourceType: ResourceType;
  description?: string | null;
  icon?: string | null;
  tags: string[];
  aliases: string[];
  targets: ProfileTargetSnapshot[];
}

export interface ProfileResourceConflictPreview {
  resolution: "reuse_existing" | "import_as_new";
  imported: ProfileResourceSnapshot;
  existing: ProfileResourceSnapshot;
  proposed: ProfileResourceSnapshot;
}

export interface ProfileImportResult {
  profileId: string;
  profileName: string;
  importedResources: number;
  reusedResources: number;
  importedLayouts: number;
  importedHistoryEvents: number;
  importedWorkspaceSequences: number;}

export interface ProfileExportResult {
  path: string;
  profileName: string;
  resourceCount: number;
  historyEventCount: number;
}

export interface CommandMap {
  resource_list: { args: { profileId?: string }; result: Resource[] };
  resource_create: { args: { resource: ResourceInput }; result: Resource };
  resource_update: { args: { id: string; patch: ResourcePatch }; result: Resource };
  resource_delete: { args: { id: string }; result: void };
  resource_search: { args: SearchRequest; result: Resource[] };
  resource_launch: { args: { request: LaunchRequest }; result: LaunchResult };
  resource_inspect: { args: { resourceId: string; forceRefresh?: boolean }; result: ResourceMetadata[] };
  profile_list: { args?: undefined; result: Profile[] };
  profile_create: { args: { name: string }; result: Profile };
  profile_theme_set: { args: { profileId: string; theme: ProfileThemeInput }; result: Profile };
  smart_collection_list: { args: { profileId: string }; result: SmartCollection[] };
  smart_collection_save: { args: { collection: SmartCollection }; result: SmartCollection };
  smart_collection_delete: { args: { profileId: string; id: string }; result: void };
  dynamic_shortcut_list: { args: { profileId: string }; result: DynamicShortcut[] };
  dynamic_shortcut_save: { args: { shortcut: DynamicShortcut }; result: DynamicShortcut };
  dynamic_shortcut_delete: { args: { profileId: string; id: string }; result: void };
  workspace_sequence_list: { args: { profileId: string }; result: WorkspaceSequence[] };
  workspace_sequence_save: { args: { sequence: WorkspaceSequence }; result: WorkspaceSequence };
  workspace_sequence_delete: { args: { profileId: string; id: string }; result: void };
  workspace_sequence_run: { args: { profileId: string; id: string; requestId: string }; result: WorkspaceSequenceRunResult };
  resource_relationship_list: { args: { profileId: string }; result: ResourceRelationship[] };
  resource_relationship_save: { args: { relationship: ResourceRelationship }; result: ResourceRelationship };
  resource_relationship_delete: { args: { profileId: string; id: string }; result: void };
  workspace_snapshot_list: { args: { profileId: string }; result: WorkspaceSnapshot[] };
  workspace_snapshot_create: { args: { profileId: string; name: string }; result: WorkspaceSnapshot };
  workspace_snapshot_delete: { args: { profileId: string; id: string }; result: void };
  workspace_snapshot_restore: { args: { profileId: string; id: string }; result: WorkspaceSnapshotRestoreResult };
  profile_switch: { args: { profileId: string }; result: Profile };
  profile_export: { args: { profileId: string; outputPath: string; includeHistory?: boolean }; result: ProfileExportResult };
  profile_import_preview: { args: { packagePath: string; targetProfileId?: string }; result: ProfileImportPreview };
  profile_import_commit: { args: { packagePath: string; strategy: ProfileImportStrategy; targetProfileId?: string; packageSha256: string; previewToken: string }; result: ProfileImportResult };
  layout_get: { args: { profileId: string; surface: DashboardLayout["surface"] }; result: DashboardLayout };
  layout_save: { args: { layout: DashboardLayout }; result: DashboardLayout };
  quick_launch_hotkey_status: { args?: undefined; result: QuickLaunchHotkeyStatus };
  quick_launch_hotkey_set: { args: { profileId: string; shortcut: string }; result: QuickLaunchHotkeyStatus };
  analytics_query: { args: UsageQuery; result: LaunchEvent[] };
  analytics_summary: { args: UsageFilter & { profileId: string }; result: AnalyticsSummary };
  usage_settings_get: { args: { profileId: string }; result: UsageSettings };
  usage_settings_set: { args: { settings: UsageSettings }; result: UsageSettings };
  usage_history_purge: { args: { profileId: string }; result: number };
  usage_export_csv: { args: { query: UsageQuery; outputPath: string }; result: UsageExportResult };
}

export interface AppEventMap {
  "quick-launch-toggle": undefined;
  "quick-launch-hotkey-status": QuickLaunchHotkeyStatus;
  "workspace-state-changed": { profileId: string };
}

/** The only frontend entry point for invoking native commands. */
export async function invokeTyped<K extends keyof CommandMap>(
  command: K,
  ...args: CommandMap[K]["args"] extends undefined
    ? [args?: undefined]
    : [args: CommandMap[K]["args"]]
): Promise<CommandMap[K]["result"]> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<CommandMap[K]["result"]>(command, args[0] as Record<string, unknown> | undefined);
}

export async function listenTyped<K extends keyof AppEventMap>(
  event: K,
  handler: (payload: AppEventMap[K]) => void,
): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<AppEventMap[K]>(event, (message) => handler(message.payload));
}

/** The only frontend entry point for emitting shared workspace events. */
export async function emitTyped<K extends keyof AppEventMap>(
  event: K,
  payload: AppEventMap[K],
): Promise<void> {
  const { emit } = await import("@tauri-apps/api/event");
  return emit(event, payload);
}
