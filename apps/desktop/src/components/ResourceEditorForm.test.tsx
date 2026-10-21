import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Resource } from "../lib/contracts";
import { ResourceEditorForm } from "./ResourceEditorForm";
const resource: Resource = { id: "r1", resourceType: "file", displayName: "Project", description: "Main workspace", defaultAction: "open", createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z", targets: [{ id: "t1", kind: "file", canonicalUri: "C:\\Project", priority: 0, arguments: [], active: true }, { id: "t2", kind: "folder", canonicalUri: "D:\\Project", priority: 1, arguments: [], active: true }], tags: ["planning"], aliases: ["roadmap"], favorite: false, launchCount: 1, category: "Work" };
let host: HTMLDivElement; let root: Root; const onCancel = vi.fn(); const onSave = vi.fn();
beforeEach(() => { host = document.createElement("div"); document.body.append(host); root = createRoot(host); onCancel.mockReset(); onSave.mockReset(); onSave.mockResolvedValue(undefined); });
afterEach(async () => { await act(async () => root.unmount()); host.remove(); });

describe("ResourceEditorForm", () => {
  it("edits resource fields and preserves all alternate targets", async () => {
    await act(async () => root.render(createElement(ResourceEditorForm, { resource, onCancel, onSave })));
    const name = host.querySelector<HTMLInputElement>('label:first-child input')!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(name, "Project archive"); name.dispatchEvent(new Event("input", { bubbles: true })); });
    const firstTarget = host.querySelector<HTMLInputElement>('[aria-label="Target 1 location"]')!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(firstTarget, "E:\\Project"); firstTarget.dispatchEvent(new Event("input", { bubbles: true })); });
    await act(async () => { host.querySelector("form")!.requestSubmit(); await Promise.resolve(); });
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ displayName: "Project archive", category: "Work", tags: ["planning"], aliases: ["roadmap"], targets: [expect.objectContaining({ id: "t1", canonicalUri: "E:\\Project" }), expect.objectContaining({ id: "t2", canonicalUri: "D:\\Project" })] }));
    expect(onCancel).toHaveBeenCalledOnce();
  });
  it("rejects blank locations without sending a patch", async () => {
    await act(async () => root.render(createElement(ResourceEditorForm, { resource, onCancel, onSave })));
    const target = host.querySelector<HTMLInputElement>('[aria-label="Target 1 location"]')!;
    await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(target, " "); target.dispatchEvent(new Event("input", { bubbles: true })); });
    await act(async () => { host.querySelector("form")!.requestSubmit(); });
    expect(onSave).not.toHaveBeenCalled();
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Each saved target needs a location");
  });
});
