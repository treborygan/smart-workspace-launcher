export type ResourceTarget = {
  id: string;
  kind: string;
  canonicalUri: string;
  priority: number;
  arguments: string[];
  workingDirectory?: string;
  active: boolean;
};

export type Resource = {
  id: string;
  resourceType: string;
  displayName: string;
  description?: string;
  icon?: string;
  defaultAction: string;
  createdUtc: string;
  updatedUtc: string;
  targets: ResourceTarget[];
  favorite?: boolean;
  category?: string;
  tags?: string[];
  launchCount?: number;
  lastLaunchedUtc?: string;
};

export type Profile = { id: string; name: string; themeId: string; settingsJson: string };
export type LaunchResult = {
  requestId: string;
  status: "requested" | "handed_off" | "failed";
  errorCode?: string;
  launchedAtUtc: string;
};

export type ResourceDraft = Pick<Resource, "resourceType" | "displayName" | "defaultAction" | "targets"> &
  Partial<Pick<Resource, "description" | "icon" | "favorite" | "category" | "tags">>;
