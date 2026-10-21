import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Resource } from "../lib/contracts";
import type { SmartCollection } from "../lib/smartCollections";
import { SmartCollectionsManager } from "./SmartCollectionsManager";

const resources: Resource[] = [
  { id: "r1", resourceType: "file", displayName: "Roadmap", defaultAction: "open", createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z", targets: [{ id: "t1", kind: "file", canonicalUri: "C:\\Roadmap.xlsx", priority: 0, arguments: [], active: true }], tags: ["planning"], aliases: [], favorite: true, launchCount: 2, category: "Work" },
  { id: "r2", resourceType: "url", displayName: "Calendar", defaultAction: "open", createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z", targets: [{ id: "t2", kind: "url", canonicalUri: "https://calendar.example", priority: 0, arguments: [], active: true }], tags: [], aliases: [], favorite: false, launchCount: 1, category: "Web" },
];
let host: HTMLDivElement; let root: Root; const onSave = vi.fn(); const onDelete = vi.fn(); const onClose = vi.fn();
const render = async (collections: SmartCollection[] = []) => act(async () => root.render(createElement(SmartCollectionsManager, { profileId: "p1", collections, resources, onSave, onDelete, onClose })));
const set = async (selector: string, value: string, elementType: "input" | "select" = "input") => {
  const element = host.querySelector(selector) as HTMLInputElement | HTMLSelectElement;
  await act(async () => { const proto = elementType === "select" ? HTMLSelectElement.prototype : HTMLInputElement.prototype; Object.getOwnPropertyDescriptor(proto, "value")!.set!.call(element, value); element.dispatchEvent(new Event(elementType === "select" ? "change" : "input", { bubbles: true })); });
};
beforeEach(() => { host = document.createElement("div"); document.body.append(host); root = createRoot(host); onSave.mockReset().mockImplementation(async (value) => value); onDelete.mockReset().mockResolvedValue(undefined); onClose.mockReset(); });
afterEach(async () => { await act(async () => root.unmount()); host.remove(); });

describe("SmartCollectionsManager", () => {
  it("builds and saves a profile-scoped live filter with preview count", async () => {
    await render();
    await set('[aria-label="Collection name"]', "Favorite work files");
    await set('[aria-label="Resource type filter"]', "file", "select");
    await set('[aria-label="Category filter"]', "Work");
    await set('[aria-label="Tag filter"]', "planning");
    await act(async () => { host.querySelector<HTMLInputElement>(".smart-collection-favorite input")!.click(); });
    expect(host.textContent).toContain("Matches 1 of 2 current resources");
    await act(async () => { host.querySelector(".smart-collection-form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })); await Promise.resolve(); });
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ profileId: "p1", name: "Favorite work files", filters: { resourceType: "file", category: "Work", tag: "planning", favoriteOnly: true } }));
    expect(host.textContent).toContain("Smart collection saved.");
  });
  it("loads a saved definition for editing and deletes only after confirmation", async () => {
    const item: SmartCollection = { id: "c1", profileId: "p1", name: "Work files", filters: { resourceType: "file" } };
    await render([item]);
    await act(async () => { host.querySelector(".smart-collection-edit")!.dispatchEvent(new MouseEvent("click", { bubbles: true })); await Promise.resolve(); });
    expect((host.querySelector('[aria-label="Collection name"]') as HTMLInputElement).value).toBe("Work files");
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent === "Delete")!.click(); await Promise.resolve(); });
    expect(confirm).toHaveBeenCalled(); expect(onDelete).toHaveBeenCalledWith("c1");
    confirm.mockRestore();
  });
});
