import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ExtensionPackagePreview, ExtensionRunResult, RegisteredExtension } from "@sw/contracts";

const { api, open } = vi.hoisted(() => ({ api: {
  isNativeApp: vi.fn(), extensionRegistry: vi.fn(), previewExtensionPackage: vi.fn(), installExtensionPackage: vi.fn(),
  setExtensionPermissions: vi.fn(), setExtensionEnabled: vi.fn(), runExtension: vi.fn(), removeExtension: vi.fn(),
}, open: vi.fn() }));
vi.mock("../lib/api", () => ({ api }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open }));
import { ExtensionsRegistry } from "./ExtensionsRegistry";

const registered: RegisteredExtension = {
  id: "com.example.focus-summary", name: "Focus summary", version: "1.0.0", sdkVersion: "1.0.0",
  description: "A safe manifest", requestedCapabilities: ["workspace_summary.read"], grantedCapabilities: [],
  enabled: false, installedUtc: "2026-10-22T00:00:00Z", moduleSha256: null,
};
const installed: RegisteredExtension = { ...registered, moduleSha256: "a".repeat(64) };
const preview: ExtensionPackagePreview = {
  manifest: { id: registered.id, name: registered.name, version: registered.version, sdkVersion: registered.sdkVersion, description: registered.description, requestedCapabilities: registered.requestedCapabilities },
  packageSha256: "b".repeat(64), moduleSha256: "a".repeat(64), moduleSizeBytes: 4096, previewToken: "review-token",
};
const suggestionResult: ExtensionRunResult = { extensionId: installed.id, suggestions: [{ title: "<img src=x onerror=alert(1)>", reason: "A plain text suggestion", resourceIds: ["res-1"] }] };
let host: HTMLDivElement; let root: Root;
const render = async (profileId = "profile-1") => act(async () => { root.render(createElement(ExtensionsRegistry, { profileId })); await Promise.resolve(); await Promise.resolve(); });
const click = async (element: HTMLButtonElement | HTMLInputElement) => act(async () => { element.click(); await Promise.resolve(); });
const choose = async () => click([...host.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent?.includes("Review .swext package"))!);
const install = async () => click([...host.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent === "Install disabled package")!);

beforeEach(() => {
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
  api.isNativeApp.mockReturnValue(true);
  api.extensionRegistry.mockResolvedValue([]);
  api.previewExtensionPackage.mockResolvedValue(preview);
  api.installExtensionPackage.mockResolvedValue(installed);
  api.setExtensionPermissions.mockImplementation(async (update) => ({ ...installed, grantedCapabilities: update.grantedCapabilities }));
  api.setExtensionEnabled.mockImplementation(async (_id, enabled) => ({ ...installed, enabled }));
  api.runExtension.mockResolvedValue(suggestionResult);
  api.removeExtension.mockResolvedValue(undefined);
  open.mockResolvedValue("C:\\Packages\\focus-summary.swext");
});
afterEach(async () => { await act(async () => root.unmount()); host.remove(); vi.resetAllMocks(); });

describe("ExtensionsRegistry", () => {
  it("selects a native .swext package and previews the manifest, module digest and requested capabilities without installing or running", async () => {
    await render();
    await choose();
    expect(open).toHaveBeenCalledWith({ multiple: false, directory: false, filters: [{ name: "Smart Workspace extension", extensions: ["swext"] }] });
    expect(api.previewExtensionPackage).toHaveBeenCalledWith({ packagePath: "C:\\Packages\\focus-summary.swext" });
    expect(host.textContent).toContain("No code has run and nothing has been installed.");
    expect(host.textContent).toContain(registered.id);
    expect(host.textContent).toContain("Read workspace summary");
    expect(host.textContent).toContain(preview.moduleSha256);
    expect(host.textContent).toContain("Install disabled package");
    expect(api.installExtensionPackage).not.toHaveBeenCalled();
    expect(api.runExtension).not.toHaveBeenCalled();
  });

  it("installs only after preview confirmation and preserves disabled, ungranted defaults", async () => {
    await render(); await choose(); await install();
    expect(api.installExtensionPackage).toHaveBeenCalledWith({ packagePath: "C:\\Packages\\focus-summary.swext", packageSha256: preview.packageSha256, previewToken: preview.previewToken });
    expect(host.querySelector<HTMLInputElement>(".extension-enable input")?.checked).toBe(false);
    expect(host.querySelector<HTMLInputElement>(".extension-grants input")?.checked).toBe(false);
    expect(host.textContent).toContain("installed disabled");
    expect(host.textContent).toContain("Enable package");
  });

  it("rejects a stale installation preview and asks for a fresh review", async () => {
    api.installExtensionPackage.mockRejectedValueOnce(new Error("Package changed since preview."));
    await render(); await choose(); await install();
    expect(host.querySelector('[aria-labelledby="extension-preview-title"]')).toBeNull();
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Package changed since preview");
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Review the package again");
  });

  it("saves only the selected declared capability and enables only after explicit action", async () => {
    api.extensionRegistry.mockResolvedValue([installed]);
    await render();
    const grant = host.querySelector<HTMLInputElement>(".extension-grants input")!;
    expect(grant.checked).toBe(false);
    await click(grant);
    expect(api.setExtensionPermissions).toHaveBeenCalledWith({ extensionId: installed.id, manifestVersion: installed.version, grantedCapabilities: ["workspace_summary.read"] });
    const enable = host.querySelector<HTMLInputElement>(".extension-enable input")!;
    expect(enable.checked).toBe(false);
    await click(enable);
    expect(api.setExtensionEnabled).toHaveBeenCalledWith(installed.id, true);
    expect(host.textContent).toContain("choose Run");
  });

  it("runs only on explicit click and renders returned values as inert text", async () => {
    api.extensionRegistry.mockResolvedValue([{ ...installed, enabled: true, grantedCapabilities: ["workspace_summary.read"] }]);
    await render("profile-1");
    const runButton = [...host.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent === "Run")!;
    expect(api.runExtension).not.toHaveBeenCalled();
    await click(runButton);
    expect(api.runExtension).toHaveBeenCalledWith(installed.id, "profile-1");
    expect(host.textContent).toContain("Text suggestions");
    expect(host.textContent).toContain("<img src=x onerror=alert(1)>");
    expect(host.querySelector("img, a, svg[onload], [onerror]")).toBeNull();
    expect(host.textContent).toContain("Nothing was launched or changed");
  });

  it("discards a run result when the active profile changes while the service is responding", async () => {
    api.extensionRegistry.mockResolvedValue([{ ...installed, enabled: true }]);
    let complete!: (value: ExtensionRunResult) => void;
    api.runExtension.mockImplementation(() => new Promise((resolve) => { complete = resolve; }));
    await render("profile-1");
    await click([...host.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent === "Run")!);
    await render("profile-2");
    await act(async () => { complete(suggestionResult); await Promise.resolve(); });
    expect(host.textContent).not.toContain("A plain text suggestion");
  });

  it("shows sandbox-unavailable errors without applying or persisting suggestions", async () => {
    api.extensionRegistry.mockResolvedValue([{ ...installed, enabled: true }]);
    api.runExtension.mockRejectedValueOnce(new Error("Sandbox runtime unavailable."));
    await render();
    await click([...host.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent === "Run")!);
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Sandbox runtime unavailable");
    expect(host.textContent).not.toContain("Text suggestions");
  });

  it("requires confirmation to remove the package and its grants, and returns focus to Keep", async () => {
    api.extensionRegistry.mockResolvedValue([installed]);
    await render();
    await click([...host.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent === "Remove")!);
    expect(api.removeExtension).not.toHaveBeenCalled();
    expect(document.activeElement?.textContent).toBe("Keep");
    await click([...host.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent === "Remove")!);
    expect(api.removeExtension).toHaveBeenCalledWith(installed.id);
    expect(host.textContent).toContain("its module and saved grants were removed");
  });

  it("shows native-only and backend package-preview errors", async () => {
    api.isNativeApp.mockReturnValue(false);
    await render();
    expect(host.textContent).toContain("native desktop app only");
    expect(api.extensionRegistry).not.toHaveBeenCalled();

    api.isNativeApp.mockReturnValue(true);
    api.previewExtensionPackage.mockRejectedValueOnce(new Error("Unsupported package entry."));
    await render(); await choose();
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Unsupported package entry");
    expect(host.querySelector('[aria-labelledby="extension-preview-title"]')).toBeNull();
  });
});
