import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
const { api, chooseProfileExportPath, chooseProfilePackage } = vi.hoisted(() => ({ api: { exportProfile: vi.fn(), previewProfileImport: vi.fn(), commitProfileImport: vi.fn(), isNativeApp: vi.fn() }, chooseProfileExportPath: vi.fn(), chooseProfilePackage: vi.fn() }));
vi.mock("../lib/api", () => ({ api }));
vi.mock("../lib/profileDialogs", () => ({ chooseProfileExportPath, chooseProfilePackage }));
import { ProfileArchiveControls } from "./ProfileArchiveControls";
import type { Profile } from "@sw/contracts";
const timestamp = "2026-01-01T00:00:00Z";
const profiles: Profile[] = [{ id: "p1", name: "Work", themeId: "system", settingsJson: "{}", createdUtc: timestamp, updatedUtc: timestamp }];
let host: HTMLDivElement; let root: Root;
const render = async () => act(async () => root.render(createElement(ProfileArchiveControls, { profiles, activeProfileId: "p1", onImported: vi.fn() })));
const clickText = async (text: string) => { const button = [...host.querySelectorAll("button")].find((item) => item.textContent?.includes(text)); if (!button) throw new Error(`Missing button: ${text}`); await act(async () => { button.click(); await Promise.resolve(); }); };
beforeEach(() => { host = document.createElement("div"); document.body.append(host); root = createRoot(host); api.isNativeApp.mockReturnValue(true); chooseProfilePackage.mockResolvedValue("C:\\work.swprofile"); chooseProfileExportPath.mockResolvedValue("C:\\export.swprofile"); });
afterEach(async () => { await act(async () => root.unmount()); host.remove(); vi.resetAllMocks(); });

describe("ProfileArchiveControls", () => {
  it("keeps package dialogs disabled in browser preview", async () => {
    api.isNativeApp.mockReturnValue(false);
    await render();
    expect(host.textContent).toContain("available in the Windows desktop app");
    expect([...host.querySelectorAll("button")].filter((button) => button.textContent?.includes("profile…")).every((button) => (button as HTMLButtonElement).disabled)).toBe(true);
    expect(chooseProfilePackage).not.toHaveBeenCalled();
  });
  it("exports through a save dialog and reports the result", async () => {
    api.exportProfile.mockResolvedValue({ path: "C:\\export.swprofile", profileName: "Work", resourceCount: 2, historyEventCount: 1 });
    await render(); await clickText("Export profile");
    expect(chooseProfileExportPath).toHaveBeenCalledWith("Work");
    expect(api.exportProfile).toHaveBeenCalledWith("p1", "C:\\export.swprofile", false);
    expect(host.textContent).toContain("Exported 2 resources from Work");
  });
  it("shows read-only collision preview and requires an explicit commit confirmation", async () => {
    api.previewProfileImport.mockResolvedValue({ profileId: "p1", profileName: "Work", packageSha256: "digest", previewToken: "opaque-token", resourceCount: 4, layoutCount: 2, historyEventCount: 3, profileIdCollision: true, profileNameCollision: true, resourceIdCollisions: ["r1"], targetIdCollisions: ["t1"], assetCount: 0, resourceConflicts: [{ resolution: "import_as_new", imported: { id: "r1", name: "Imported report", resourceType: "file", description: "From package", tags: ["finance"], aliases: [], targets: [{ id: "t1", kind: "file", value: "C:\\Package\\report.xlsx", priority: 0, arguments: [], active: true }] }, existing: { id: "r1", name: "Existing report", resourceType: "file", description: "Local copy", tags: ["local"], aliases: [], targets: [{ id: "t1", kind: "file", value: "D:\\Local\\report.xlsx", priority: 0, arguments: [], active: true }] }, proposed: { id: "r1-new", name: "Imported report", resourceType: "file", description: "From package", tags: ["finance"], aliases: [], targets: [{ id: "t1-new", kind: "file", value: "C:\\Package\\report.xlsx", priority: 0, arguments: [], active: true }] } }] });
    api.commitProfileImport.mockResolvedValue({ profileId: "p2", profileName: "Work (2)", importedResources: 4, reusedResources: 0, importedLayouts: 2, importedHistoryEvents: 3 });
    await render(); await clickText("Import profile");
    expect(api.previewProfileImport).toHaveBeenCalledWith("C:\\work.swprofile");
    expect(host.textContent).toContain("This preview is read-only. No data has changed.");
    expect(host.textContent).toContain("Resource IDs: 1");
    expect(host.textContent).toContain("Existing report"); expect(host.textContent).toContain("Imported report"); expect(host.textContent).toContain("Imported values"); expect(host.textContent).toContain("Duplicate creates this as a new resource");
    expect(host.textContent).toContain("digest");
    expect(api.commitProfileImport).not.toHaveBeenCalled();
    await clickText("Review commit");
    expect(host.textContent).toContain("Confirm import");
    expect(api.commitProfileImport).not.toHaveBeenCalled();
    await clickText("Confirm import");
    expect(api.commitProfileImport).toHaveBeenCalledWith("C:\\work.swprofile", "duplicate", undefined, "digest", "opaque-token");
    expect(host.textContent).toContain("Imported 4 resources into Work (2)");
  });
  it("requires strategy and target profile selection before replacement commit", async () => {
    const snapshot = { id: "r1", name: "Example", resourceType: "file" as const, description: null, icon: null, tags: [], aliases: [], targets: [{ id: "t1", kind: "file" as const, value: "C:\\Example.txt", priority: 0, arguments: [], active: true }] };
    api.previewProfileImport.mockResolvedValue({ profileId: "package-id", profileName: "Imported", packageSha256: "digest", previewToken: "token", resourceCount: 1, layoutCount: 0, historyEventCount: 0, profileIdCollision: false, profileNameCollision: false, resourceIdCollisions: [], targetIdCollisions: [], resourceConflicts: [{ resolution: "import_as_new", imported: snapshot, existing: snapshot, proposed: snapshot }], assetCount: 0 });
    await render(); await clickText("Import profile");
    const select = [...host.querySelectorAll("select")].find((item) => item.options[0]?.value === "duplicate")!;
    await act(async () => { select.value = "replace"; select.dispatchEvent(new Event("change", { bubbles: true })); });
    expect(host.textContent).toContain("Replace an existing profile’s contents");
    expect(host.textContent).toContain("Replace clears profile membership, then imports this as a new resource");
    expect(host.textContent).toContain("Target profile");
    await clickText("Review commit");
    expect(host.textContent).toContain("Existing resources and layouts");
    expect(api.commitProfileImport).not.toHaveBeenCalled();
  });
  it("surfaces validation failures without attempting import", async () => {
    api.previewProfileImport.mockRejectedValue(new Error("Invalid package manifest"));
    await render(); await clickText("Import profile");
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Invalid package manifest");
    expect(api.commitProfileImport).not.toHaveBeenCalled();
  });
});
