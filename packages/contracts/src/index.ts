/** Stable JSON boundary shared by the React client and Tauri commands. */
export type ResourceType = "file" | "folder" | "app" | "shortcut" | "url";
export type TargetKind = ResourceType;
export type LaunchStatus = "requested" | "handed_off" | "failed";
export type LaunchSource = "dashboard" | "quick_launch" | "dock" | "explorer" | "other";

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

export interface CommandMap {
  resource_list: { args: { profileId?: string }; result: Resource[] };
  resource_create: { args: { resource: ResourceInput }; result: Resource };
  resource_update: { args: { id: string; patch: ResourcePatch }; result: Resource };
  resource_delete: { args: { id: string }; result: void };
  resource_search: { args: SearchRequest; result: Resource[] };
  resource_launch: { args: { request: LaunchRequest }; result: LaunchResult };
  profile_list: { args?: undefined; result: Profile[] };
  profile_create: { args: { name: string }; result: Profile };
  profile_switch: { args: { profileId: string }; result: Profile };
  layout_get: { args: { profileId: string; surface: DashboardLayout["surface"] }; result: DashboardLayout };
  layout_save: { args: { layout: DashboardLayout }; result: DashboardLayout };
  quick_launch_hotkey_status: { args?: undefined; result: QuickLaunchHotkeyStatus };
  quick_launch_hotkey_set: { args: { profileId: string; shortcut: string }; result: QuickLaunchHotkeyStatus };
  analytics_query: { args: { profileId?: string; limit?: number }; result: LaunchEvent[] };
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
