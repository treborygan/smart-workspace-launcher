import { invokeTyped } from "@sw/contracts";
import type { ResourcePatch } from "@sw/contracts";
import type { LaunchResult, Profile, Resource, ResourceDraft, ResourceTarget } from "./contracts";

const STORAGE_KEY = "swl-ui-demo-resources-v1";
const isNative = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

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
    if (isNative()) return invokeTyped("resource_create", { resource: draft });
    const resources = readDemo();
    const targets: ResourceTarget[] = draft.targets.map((target) => ({ id: target.id ?? crypto.randomUUID(), kind: target.kind, canonicalUri: target.canonicalUri, priority: target.priority, arguments: target.arguments, workingDirectory: target.workingDirectory || undefined, active: target.active }));
    const created: Resource = { ...draft, defaultAction: draft.defaultAction || "open", id: crypto.randomUUID(), createdUtc: new Date().toISOString(), updatedUtc: new Date().toISOString(), targets, tags: draft.tags || [], aliases: draft.aliases || [], favorite: false, launchCount: 0 };
    writeDemo([...resources, created]); return created;
  },
  async update(id: string, patch: ResourcePatch): Promise<Resource> {
    if (isNative()) return invokeTyped("resource_update", { id, patch });
    const resources: Resource[] = readDemo().map((r) => {
      if (r.id !== id) return r;
      const next = { ...r, ...patch, updatedUtc: new Date().toISOString() };
      const targets: ResourceTarget[] = (patch.targets ?? r.targets).map((target) => ({ id: target.id ?? crypto.randomUUID(), kind: target.kind, canonicalUri: target.canonicalUri, priority: target.priority, arguments: target.arguments, workingDirectory: target.workingDirectory || undefined, active: target.active }));
      return { ...next, targets };
    });
    writeDemo(resources); return resources.find((r) => r.id === id)!;
  },
  async remove(id: string): Promise<void> {
    if (isNative()) return invokeTyped("resource_delete", { id });
    writeDemo(readDemo().filter((r) => r.id !== id));
  },
  async launch(id: string): Promise<LaunchResult> {
    if (isNative()) return invokeTyped("resource_launch", { request: { requestId: crypto.randomUUID(), resourceId: id, source: "dashboard" } });
    throw new Error("Native launch is available in the Windows desktop app. This browser preview does not open local resources.");
  },
  async profiles(): Promise<Profile[]> {
    if (isNative()) return invokeTyped("profile_list");
    return [{ id: "default", name: "Personal", themeId: "system", settingsJson: "{}", createdUtc: now, updatedUtc: now }];
  },
};
