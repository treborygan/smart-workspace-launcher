import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ResourceImportScanResult } from "@sw/contracts";

const { api, open } = vi.hoisted(() => ({
  api: { isNativeApp: vi.fn(), scanResourceImport: vi.fn(), commitResourceImport: vi.fn() },
  open: vi.fn(),
}));
vi.mock("../lib/api", () => ({ api }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open }));
import { ResourceImportAssistant } from "./ResourceImportAssistant";

const preview: ResourceImportScanResult = {
  profileId: "p1",
  source: { kind: "directory", path: "C:\\Work" },
  scanToken: "scan-token",
  ignoredCount: 2,
  candidates: [
    { candidateId: "c1", resourceType: "file", displayName: "Roadmap.xlsx", targetUri: "C:\\Work\\Roadmap.xlsx", sourceLabel: "Directory", provenance: "C:\\Work\\Roadmap.xlsx", existingResourceId: "r1" },
    { candidateId: "c2", resourceType: "url", displayName: "Project site", targetUri: "https://example.test", sourceLabel: "Internet shortcut", provenance: "C:\\Work\\project.url" },
  ],
};

let host: HTMLDivElement;
let root: Root;
const render = async (profileId = "p1", onImported = vi.fn()) => act(async () => root.render(createElement(ResourceImportAssistant, { profileId, onImported })));

beforeEach(() => {
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
  api.isNativeApp.mockReset(); api.scanResourceImport.mockReset(); api.commitResourceImport.mockReset(); open.mockReset();
  api.isNativeApp.mockReturnValue(true); open.mockResolvedValue("C:\\Work"); api.scanResourceImport.mockResolvedValue(preview);
  api.commitResourceImport.mockResolvedValue({ profileId: "p1", importedCount: 1, skippedCount: 1 });
});
afterEach(async () => { await act(async () => root.unmount()); host.remove(); });

describe("ResourceImportAssistant", () => {
  it("does not scan in browser preview", async () => {
    api.isNativeApp.mockReturnValue(false);
    await render();
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent?.includes("Choose directory"))!.click(); });
    expect(host.textContent).toContain("available in the Windows desktop app");
    expect(api.scanResourceImport).not.toHaveBeenCalled();
  });

  it("requires explicit scan and commit, previews provenance, selection and duplicate policy", async () => {
    const onImported = vi.fn();
    await render("p1", onImported);
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent?.includes("Choose directory"))!.click(); });
    expect(open).toHaveBeenCalledWith({ multiple: false, directory: true });
    expect(api.scanResourceImport).not.toHaveBeenCalled();
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent === "Scan selected source")!.click(); });
    expect(api.scanResourceImport).toHaveBeenCalledWith({ profileId: "p1", source: { kind: "directory", path: "C:\\Work" } });
    expect(host.textContent).toContain("Nothing has been imported");
    expect(host.textContent).toContain("Already in profile");
    expect(host.textContent).toContain("Internet shortcut");
    expect(api.commitResourceImport).not.toHaveBeenCalled();
    await act(async () => { [...host.querySelectorAll<HTMLInputElement>("input[type=radio]")].find((input) => input.value === "create_duplicate")!.click(); });
    const checkboxes = [...host.querySelectorAll<HTMLInputElement>("tbody input[type=checkbox]")];
    await act(async () => { checkboxes[1].click(); });
    expect(host.textContent).toContain("1 selected");
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent === "Import 1 selected")!.click(); });
    expect(api.commitResourceImport).toHaveBeenCalledWith({ profileId: "p1", source: preview.source, scanToken: "scan-token", candidateIds: ["c1"], duplicatePolicy: "create_duplicate" });
    expect(onImported).toHaveBeenCalledWith("Imported 1 resource; skipped 1.");
  });

  it("clears a preview after profile changes and does not reuse old candidate selection", async () => {
    await render("p1");
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent?.includes("Choose directory"))!.click(); });
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent === "Scan selected source")!.click(); });
    await render("p2");
    expect(host.textContent).not.toContain("Review candidates");
    expect(host.textContent).not.toContain("C:\\Work");
  });

  it("discards a scan response that finishes after the active profile changes", async () => {
    let finishScan!: (result: ResourceImportScanResult) => void;
    api.scanResourceImport.mockImplementation(() => new Promise((resolve) => { finishScan = resolve; }));
    await render("p1");
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent?.includes("Choose directory"))!.click(); });
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent === "Scan selected source")!.click(); await Promise.resolve(); });
    await render("p2");
    await act(async () => { finishScan(preview); await Promise.resolve(); });
    expect(host.textContent).not.toContain("Review candidates");
    expect(host.textContent).not.toContain("Roadmap.xlsx");
  });

  it("locks selection, duplicate policy and source controls while commit is pending", async () => {
    let finishCommit!: (result: { profileId: string; importedCount: number; skippedCount: number }) => void;
    api.commitResourceImport.mockImplementation(() => new Promise((resolve) => { finishCommit = resolve; }));
    await render();
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent?.includes("Choose directory"))!.click(); });
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent === "Scan selected source")!.click(); });
    await act(async () => { [...host.querySelectorAll("button")].find((button) => button.textContent === "Import 2 selected")!.click(); await Promise.resolve(); });
    expect([...host.querySelectorAll<HTMLInputElement>("tbody input[type=checkbox]")].every((checkbox) => checkbox.disabled)).toBe(true);
    expect((host.querySelector(".resource-import-policy") as HTMLFieldSetElement).disabled).toBe(true);
    expect((host.querySelector("select[aria-label='Source type']") as HTMLSelectElement).disabled).toBe(true);
    expect([...host.querySelectorAll("button")].find((button) => button.textContent?.startsWith("Choose"))?.disabled).toBe(true);
    await act(async () => { finishCommit({ profileId: "p1", importedCount: 1, skippedCount: 1 }); await Promise.resolve(); });
    expect(host.textContent).toContain("Imported 1 resource; skipped 1.");
  });
});
