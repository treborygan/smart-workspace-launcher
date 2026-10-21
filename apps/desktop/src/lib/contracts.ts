import type { Resource as SharedResource, ResourceInput } from "@sw/contracts";

export type { DashboardLayout, LaunchEvent, LaunchRequest, LaunchResult, Profile, ResourceTarget } from "@sw/contracts";
export type Resource = SharedResource & { category?: string | null };
export type ResourceDraft = ResourceInput;
