import type { Resource as SharedResource, ResourceInput } from "@sw/contracts";

export type { AppEventMap, DashboardLayout, LaunchEvent, LaunchRequest, LaunchResult, Profile, QuickLaunchHotkeyStatus, ResourceTarget } from "@sw/contracts";
export type Resource = SharedResource & { category?: string | null };
export type ResourceDraft = ResourceInput;
