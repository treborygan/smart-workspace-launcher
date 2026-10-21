import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Resource } from "../lib/contracts";
import type { DynamicShortcut } from "@sw/contracts";
import { DynamicShortcutsManager } from "./DynamicShortcutsManager";
const resources: Resource[] = [{ id: "r1", resourceType: "file", displayName: "Report", defaultAction: "open", createdUtc: "2026-01-01", updatedUtc: "2026-01-01", targets: [{ id: "t1", kind: "file", canonicalUri: "C:\\Reports\\Q3.xlsx", priority: 0, arguments: [], active: true }, { id: "t2", kind: "file", canonicalUri: "D:\\Backup\\Q3.xlsx", priority: 1, arguments: [], active: false }], tags: [], aliases: [], favorite: false, launchCount: 0 }];
let host: HTMLDivElement; let root: Root; const onSave = vi.fn(); const onDelete = vi.fn(); const onClose = vi.fn();
const render = async (shortcuts: DynamicShortcut[] = []) => act(async () => root.render(createElement(DynamicShortcutsManager, { profileId: "p1", resources, shortcuts, onSave, onDelete, onClose })));
const set = async (selector: string, value: string, select = false) => { const element = host.querySelector(selector) as HTMLInputElement | HTMLSelectElement; await act(async () => { const prototype = select ? HTMLSelectElement.prototype : HTMLInputElement.prototype; Object.getOwnPropertyDescriptor(prototype, "value")!.set!.call(element, value); element.dispatchEvent(new Event(select ? "change" : "input", { bubbles: true })); }); };
beforeEach(() => { host = document.createElement("div"); document.body.append(host); root = createRoot(host); onSave.mockReset().mockImplementation(async (shortcut) => shortcut); onDelete.mockReset().mockResolvedValue(undefined); onClose.mockReset(); });
afterEach(async () => { await act(async () => root.unmount()); host.remove(); });
describe("DynamicShortcutsManager", () => {
  it("saves only a named resource and its selected active target", async () => {
    await render();
    await set('[aria-label="Shortcut name"]', "Quarterly report");
    await set('[aria-label="Shortcut resource"]', "r1", true);
    await set('[aria-label="Shortcut target"]', "t1", true);
    await act(async () => { host.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })); await Promise.resolve(); });
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ profileId: "p1", name: "Quarterly report", resourceId: "r1", targetId: "t1" }));
    expect(host.textContent).toContain("Quick Launch shortcut saved.");
  });
  it("does not offer inactive targets as executable choices", async () => {
    await render(); await set('[aria-label="Shortcut resource"]', "r1", true);
    const options = [...host.querySelectorAll<HTMLSelectElement>('[aria-label="Shortcut target"] option')].map((option) => option.value);
    expect(options).toContain("t1"); expect(options).not.toContain("t2");
  });
  it("loads an existing shortcut for editing and confirms deletion", async () => {
    const saved = { id: "ds1", profileId: "p1", name: "Weekly", resourceId: "r1", targetId: "t1" };
    await render([saved]);
    await act(async () => { (host.querySelector(".smart-collection-edit") as HTMLButtonElement).click(); await Promise.resolve(); });
    expect((host.querySelector('[aria-label="Shortcut name"]') as HTMLInputElement).value).toBe("Weekly");
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent === "Delete")!.click(); await Promise.resolve(); });
    expect(onDelete).toHaveBeenCalledWith("ds1"); confirm.mockRestore();
  });
});
