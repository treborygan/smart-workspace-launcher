import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ResourceRelationship, WorkspaceSnapshot } from "@sw/contracts";
import type { Resource } from "../lib/contracts";
import { WorkspaceMapManager } from "./WorkspaceMapManager";

const resources: Resource[] = [
  { id: "r1", resourceType: "file", displayName: "Roadmap", defaultAction: "open", createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z", targets: [], tags: [], aliases: [], favorite: false, launchCount: 0 },
  { id: "r2", resourceType: "url", displayName: "Calendar", defaultAction: "open", createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z", targets: [], tags: [], aliases: [], favorite: false, launchCount: 0 },
];
const relationship: ResourceRelationship = { id: "edge1", profileId: "p1", sourceResourceId: "r1", targetResourceId: "r2", label: "supports" };
const snapshot: WorkspaceSnapshot = { id: "snap1", profileId: "p1", name: "Planning", createdUtc: "2026-01-01T12:00:00Z", resourceIds: ["r1", "r2"], dashboard: { profileId: "p1", surface: "dashboard", version: 1, cards: [] }, dock: { profileId: "p1", surface: "dock", version: 1, cards: [] } };
let host: HTMLDivElement; let root: Root;
const onSelectResource = vi.fn(); const onSaveRelationship = vi.fn(); const onDeleteRelationship = vi.fn(); const onCreateSnapshot = vi.fn(); const onDeleteSnapshot = vi.fn(); const onRestoreSnapshot = vi.fn(); const onClose = vi.fn();
const render = async (relationships: ResourceRelationship[] = [], snapshots: WorkspaceSnapshot[] = []) => act(async () => root.render(createElement(WorkspaceMapManager, { profileId: "p1", resources, relationships, snapshots, onClose, onSelectResource, onSaveRelationship, onDeleteRelationship, onCreateSnapshot, onDeleteSnapshot, onRestoreSnapshot })));
const set = async (selector: string, value: string, type: "input" | "select" = "input") => {
  const element = host.querySelector(selector) as HTMLInputElement | HTMLSelectElement;
  await act(async () => { const proto = type === "select" ? HTMLSelectElement.prototype : HTMLInputElement.prototype; Object.getOwnPropertyDescriptor(proto, "value")!.set!.call(element, value); element.dispatchEvent(new Event(type === "select" ? "change" : "input", { bubbles: true })); });
};
beforeEach(() => { host = document.createElement("div"); document.body.append(host); root = createRoot(host); onSelectResource.mockReset(); onSaveRelationship.mockReset().mockImplementation(async (value) => value); onDeleteRelationship.mockReset().mockResolvedValue(undefined); onCreateSnapshot.mockReset().mockImplementation(async (name: string) => ({ ...snapshot, name })); onDeleteSnapshot.mockReset().mockResolvedValue(undefined); onRestoreSnapshot.mockReset().mockResolvedValue({ snapshotId: "snap1", restoredLayouts: 2, missingResourceIds: [] }); onClose.mockReset(); });
afterEach(async () => { await act(async () => root.unmount()); host.remove(); });

describe("WorkspaceMapManager", () => {
  it("saves an undirected resource relationship and connects graph selection to Explorer", async () => {
    await render();
    await set('[aria-label="First resource"]', "r1", "select");
    await set('[aria-label="Related resource"]', "r2", "select");
    await set('[aria-label="Relationship label"]', "supports");
    await act(async () => host.querySelector<HTMLButtonElement>(".workspace-map-create-link .button.primary")!.click());
    expect(onSaveRelationship).toHaveBeenCalledWith(expect.objectContaining({ profileId: "p1", sourceResourceId: "r1", targetResourceId: "r2", label: "supports" }));
    await render([relationship]);
    await act(async () => host.querySelector<SVGGElement>('[aria-label="Select Roadmap"]')!.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    expect(host.textContent).toContain("1 related resource");
    expect(host.textContent).toContain("Calendar");
    await act(async () => host.querySelector<HTMLButtonElement>(".workspace-graph-inspector .button")!.click());
    expect(onSelectResource).toHaveBeenCalledWith("r1");
  });

  it("creates, restores, and deletes named layout snapshots", async () => {
    await render([], [snapshot]);
    await set('[aria-label="Snapshot name"]', "End of day");
    await act(async () => host.querySelector<HTMLButtonElement>(".workspace-snapshot-create .button")!.click());
    expect(onCreateSnapshot).toHaveBeenCalledWith("End of day");
    await act(async () => host.querySelector<HTMLButtonElement>(".workspace-snapshot-list .button.secondary")!.click());
    expect(onRestoreSnapshot).toHaveBeenCalledWith("snap1");
    expect(host.querySelector('[role="status"]')?.textContent).toContain("layouts restored");
    await act(async () => host.querySelector<HTMLButtonElement>('[aria-label="Delete Planning"]')!.click());
    expect(onDeleteSnapshot).toHaveBeenCalledWith("snap1");
  });

  it("supports keyboard activation and reports missing references after restore", async () => {
    onRestoreSnapshot.mockResolvedValueOnce({ snapshotId: "snap1", restoredLayouts: 2, missingResourceIds: ["gone"] });
    await render([relationship], [snapshot]);
    const node = host.querySelector<SVGGElement>('[aria-label="Select Roadmap"]')!;
    await act(async () => node.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })));
    expect(host.textContent).toContain("1 related resource");
    await act(async () => host.querySelector<HTMLButtonElement>(".workspace-snapshot-list .button.secondary")!.click());
    expect(host.querySelector('[role="status"]')?.textContent).toContain("1 saved resource reference");
  });

  it("bounds dense profiles to a readable focused neighborhood", async () => {
    const manyResources = Array.from({ length: 80 }, (_, index) => ({ ...resources[index % resources.length], id: `r${index}`, displayName: `Resource ${String(index).padStart(2, "0")}` }));
    const manyRelationships = Array.from({ length: 79 }, (_, index) => ({ id: `e${index}`, profileId: "p1", sourceResourceId: "r0", targetResourceId: `r${index + 1}`, label: "related" }));
    await act(async () => root.render(createElement(WorkspaceMapManager, { profileId: "p1", resources: manyResources, relationships: manyRelationships, snapshots: [], onClose, onSelectResource, onSaveRelationship, onDeleteRelationship, onCreateSnapshot, onDeleteSnapshot, onRestoreSnapshot })));
    expect(host.querySelectorAll(".workspace-graph-node")).toHaveLength(17);
    expect(host.textContent).toContain("Showing the selected resource and up to 16 directly related resources");
    await set('[aria-label="Focus resource"]', "r79", "select");
    expect(host.querySelectorAll(".workspace-graph-node")).toHaveLength(2);
    expect(host.querySelector('[aria-label="Select Resource 79"]')).toBeTruthy();
  });

  it("shows accessible empty states and blocks self-links", async () => {
    const one = resources.slice(0, 1);
    await act(async () => root.render(createElement(WorkspaceMapManager, { profileId: "p1", resources: one, relationships: [], snapshots: [], onClose, onSelectResource, onSaveRelationship, onDeleteRelationship, onCreateSnapshot, onDeleteSnapshot, onRestoreSnapshot })));
    expect(host.textContent).toContain("No snapshots yet");
    expect(host.querySelector<HTMLButtonElement>(".workspace-map-create-link .button.primary")!.disabled).toBe(true);
    expect(host.querySelector('[aria-label="Related resource graph"]')).toBeTruthy();
    await render();
    await set('[aria-label="First resource"]', "r1", "select");
    await set('[aria-label="Related resource"]', "r1", "select");
    await set('[aria-label="Relationship label"]', "self");
    await act(async () => host.querySelector<HTMLButtonElement>(".workspace-map-create-link .button.primary")!.click());
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("cannot be linked to itself");
    expect(onSaveRelationship).not.toHaveBeenCalled();
  });
});
