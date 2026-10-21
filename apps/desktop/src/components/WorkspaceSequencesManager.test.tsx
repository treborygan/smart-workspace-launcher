import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceSequence, WorkspaceSequenceRunResult } from "@sw/contracts";
import type { Resource } from "../lib/contracts";
import { WorkspaceSequencesManager } from "./WorkspaceSequencesManager";

const resources: Resource[] = [
  { id: "r1", resourceType: "file", displayName: "Roadmap", defaultAction: "open", createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z", targets: [{ id: "t1", kind: "file", canonicalUri: "C:\\Roadmap.xlsx", priority: 0, arguments: [], active: true }, { id: "t1b", kind: "file", canonicalUri: "D:\\Roadmap.xlsx", priority: 1, arguments: [], active: true }], tags: [], aliases: [], favorite: true, launchCount: 1 },
  { id: "r2", resourceType: "url", displayName: "Calendar", defaultAction: "open", createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z", targets: [{ id: "t2", kind: "url", canonicalUri: "https://calendar.example", priority: 0, arguments: [], active: true }], tags: [], aliases: [], favorite: false, launchCount: 0 },
];
let host: HTMLDivElement; let root: Root;
const onSave = vi.fn(); const onDelete = vi.fn(); const onClose = vi.fn(); const onRun = vi.fn();
const render = async (sequences: WorkspaceSequence[] = []) => act(async () => root.render(createElement(WorkspaceSequencesManager, { profileId: "p1", sequences, resources, onSave, onDelete, onClose, onRun })));
const set = async (selector: string, value: string, type: "input" | "select" = "input") => {
  const element = host.querySelector(selector) as HTMLInputElement | HTMLSelectElement;
  await act(async () => { const proto = type === "select" ? HTMLSelectElement.prototype : HTMLInputElement.prototype; Object.getOwnPropertyDescriptor(proto, "value")!.set!.call(element, value); element.dispatchEvent(new Event(type === "select" ? "change" : "input", { bubbles: true })); });
};
beforeEach(() => { host = document.createElement("div"); document.body.append(host); root = createRoot(host); onSave.mockReset().mockImplementation(async (value) => value); onDelete.mockReset().mockResolvedValue(undefined); onClose.mockReset(); onRun.mockReset(); });
afterEach(async () => { await act(async () => root.unmount()); host.remove(); });

describe("WorkspaceSequencesManager", () => {
  it("creates an ordered, profile-scoped resource sequence and selects optional target", async () => {
    await render();
    await set('[aria-label="Sequence name"]', "Start planning");
    await act(async () => host.querySelector<HTMLButtonElement>(".workspace-sequence-steps > button")!.click());
    await set('[aria-label="Resource for step 1"]', "r1", "select");
    await set('[aria-label="Target for step 1"]', "t1b", "select");
    await act(async () => host.querySelector<HTMLButtonElement>(".workspace-sequence-form > footer .button.primary")!.click());
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ profileId: "p1", name: "Start planning", steps: [expect.objectContaining({ resourceId: "r1", targetId: "t1b" })] }));
  });

  it("orders steps and presents a partial failure result when a run stops", async () => {
    const saved: WorkspaceSequence = { id: "s1", profileId: "p1", name: "Morning setup", steps: [{ id: "st1", resourceId: "r1" }, { id: "st2", resourceId: "r2" }] };
    const result: WorkspaceSequenceRunResult = { sequenceId: "s1", status: "failed", totalSteps: 2, completedSteps: 1, stoppedAtStepId: "st2", steps: [{ stepId: "st1", resourceId: "r1", result: { requestId: "q1", status: "handed_off", launchedAtUtc: "now" } }, { stepId: "st2", resourceId: "r2", result: { requestId: "q2", status: "failed", launchedAtUtc: "now", message: "Target is missing" } }] };
    onRun.mockResolvedValue(result);
    await render([saved]);
    await act(async () => host.querySelector<HTMLButtonElement>(".workspace-sequence-edit")!.click());
    await act(async () => host.querySelector<HTMLButtonElement>('[aria-label="Move step 2 earlier"]')!.click());
    expect(host.querySelector<HTMLSelectElement>('[aria-label="Resource for step 1"]')!.value).toBe("r2");
    await act(async () => { host.querySelectorAll<HTMLButtonElement>(".workspace-sequence-row .button")[0].click(); await Promise.resolve(); });
    expect(onRun).toHaveBeenCalledWith("s1");
    expect(host.textContent).toContain("1 of 2 steps completed.");
    expect(host.textContent).toContain("Stopped at step 2. Target is missing");
  });

  it("disables running a saved sequence while its ordered draft has unsaved changes", async () => {
    const saved: WorkspaceSequence = { id: "s1", profileId: "p1", name: "Morning setup", steps: [{ id: "st1", resourceId: "r1" }] };
    await render([saved]);
    await act(async () => host.querySelector<HTMLButtonElement>(".workspace-sequence-edit")!.click());
    expect(host.querySelector<HTMLButtonElement>(".workspace-sequence-form footer .button.secondary")!.disabled).toBe(false);
    await set('[aria-label="Sequence name"]', "Changed setup");
    expect(host.querySelector<HTMLButtonElement>(".workspace-sequence-form footer .button.secondary")!.disabled).toBe(true);
  });

  it("clears a deleted current sequence and reports deletion failures", async () => {
    const saved: WorkspaceSequence = { id: "s1", profileId: "p1", name: "Morning setup", steps: [{ id: "st1", resourceId: "r1" }] };
    await render([saved]);
    await act(async () => host.querySelector<HTMLButtonElement>(".workspace-sequence-edit")!.click());
    onDelete.mockRejectedValueOnce(new Error("Database is busy"));
    await act(async () => { host.querySelector<HTMLButtonElement>('[aria-label="Delete Morning setup"]')!.click(); await Promise.resolve(); });
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Database is busy");
    onDelete.mockResolvedValueOnce(undefined);
    await act(async () => { host.querySelector<HTMLButtonElement>('[aria-label="Delete Morning setup"]')!.click(); await Promise.resolve(); });
    expect(host.querySelector<HTMLInputElement>('[aria-label="Sequence name"]')?.value).toBe("");
  });

  it("requires a nonempty name and at least one step", async () => {
    await render();
    await act(async () => host.querySelector<HTMLButtonElement>(".workspace-sequence-form > footer .button.primary")!.click());
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Enter a name");
    expect(onSave).not.toHaveBeenCalled();
  });
});
