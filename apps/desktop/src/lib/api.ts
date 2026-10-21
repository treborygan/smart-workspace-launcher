import type { LaunchResult, Profile, Resource, ResourceDraft } from "./contracts";

const STORAGE_KEY = "swl-ui-demo-resources-v1";
const isNative = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
  return tauriInvoke<T>(command, args);
}

const now = new Date().toISOString();
const demoResources: Resource[] = [
  { id: "a1", resourceType: "application", displayName: "Design review", description: "Product design and feedback", defaultAction: "open", createdUtc: now, updatedUtc: now, category: "Work", favorite: true, launchCount: 18, targets: [{ id: "t1", kind: "path", canonicalUri: "C:\\Workspace\\Design review", priority: 0, arguments: [], active: true }] },
  { id: "a2", resourceType: "file", displayName: "Q3 Planning.xlsx", description: "Quarterly planning workbook", defaultAction: "open", createdUtc: now, updatedUtc: now, category: "Work", favorite: true, launchCount: 12, targets: [{ id: "t2", kind: "path", canonicalUri: "C:\\Workspace\\Q3 Planning.xlsx", priority: 0, arguments: [], active: true }] },
  { id: "a3", resourceType: "url", displayName: "Notion", description: "Team workspace", defaultAction: "open", createdUtc: now, updatedUtc: now, category: "Web", favorite: true, launchCount: 9, targets: [{ id: "t3", kind: "url", canonicalUri: "https://www.notion.so", priority: 0, arguments: [], active: true }] },
  { id: "a4", resourceType: "folder", displayName: "Shared drive", description: "Shared team files", defaultAction: "open", createdUtc: now, updatedUtc: now, category: "Work", favorite: true, launchCount: 7, targets: [{ id: "t4", kind: "path", canonicalUri: "Z:\\Team", priority: 0, arguments: [], active: true }] },
  { id: "a5", resourceType: "url", displayName: "Calendar", description: "Your schedule", defaultAction: "open", createdUtc: now, updatedUtc: now, category: "Web", favorite: true, launchCount: 22, targets: [{ id: "t5", kind: "url", canonicalUri: "https://calendar.google.com", priority: 0, arguments: [], active: true }] },
  { id: "a6", resourceType: "application", displayName: "Microsoft Excel", description: "Spreadsheets", defaultAction: "open", createdUtc: now, updatedUtc: now, category: "Apps", favorite: false, launchCount: 31, targets: [{ id: "t6", kind: "path", canonicalUri: "C:\\Program Files\\Microsoft Office\\root\\Office16\\EXCEL.EXE", priority: 0, arguments: [], active: true }] },
];

function readDemo(): Resource[] {
  try { return JSON.parse(localStorage.getItem(STORAGE_KEY) || "null") || demoResources; }
  catch { return demoResources; }
}
function writeDemo(resources: Resource[]) { localStorage.setItem(STORAGE_KEY, JSON.stringify(resources)); }

export const api = {
  async resources(): Promise<Resource[]> {
    if (isNative()) return invoke("resource_list");
    return readDemo();
  },
  async create(draft: ResourceDraft): Promise<Resource> {
    const resourceInput = { ...draft, resourceType: draft.resourceType === "application" ? "app" : draft.resourceType };
    if (isNative()) return invoke("resource_create", { resource: resourceInput });
    const resources = readDemo();
    const created: Resource = { ...draft, id: crypto.randomUUID(), createdUtc: new Date().toISOString(), updatedUtc: new Date().toISOString(), targets: draft.targets };
    writeDemo([...resources, created]); return created;
  },
  async update(id: string, patch: Partial<Resource>): Promise<Resource> {
    if (isNative()) return invoke("resource_update", { id, patch });
    const resources = readDemo().map((r) => r.id === id ? { ...r, ...patch, updatedUtc: new Date().toISOString() } : r);
    writeDemo(resources); return resources.find((r) => r.id === id)!;
  },
  async remove(id: string): Promise<void> {
    if (isNative()) return invoke("resource_delete", { id });
    writeDemo(readDemo().filter((r) => r.id !== id));
  },
  async launch(id: string): Promise<LaunchResult> {
    if (isNative()) return invoke("resource_launch", { request: { requestId: crypto.randomUUID(), resourceId: id, source: "dashboard" } });
    throw new Error("Native launch is available in the Windows desktop app. This browser preview does not open local resources.");
  },
  async profiles(): Promise<Profile[]> {
    if (isNative()) return invoke("profile_list");
    return [{ id: "default", name: "Personal", themeId: "system", settingsJson: "{}" }];
  },
};
