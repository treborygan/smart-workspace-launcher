import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ExtensionManifest, RegisteredExtension } from "@sw/contracts";

const { api } = vi.hoisted(() => ({ api: {
  isNativeApp: vi.fn(), extensionRegistry: vi.fn(), registerExtension: vi.fn(),
  setExtensionPermissions: vi.fn(), setExtensionEnabled: vi.fn(), removeExtension: vi.fn(),
} }));
vi.mock("../lib/api", () => ({ api }));
import { ExtensionsRegistry } from "./ExtensionsRegistry";

const manifest: ExtensionManifest = { id: "com.example.focus-summary", name: "Focus summary", version: "1.0.0", sdkVersion: "1.0.0", description: "A safe manifest", requestedCapabilities: ["workspace_summary.read"] };
const registered: RegisteredExtension = { ...manifest, grantedCapabilities: [], enabled: false, installedUtc: "2026-10-22T00:00:00Z" };
let host: HTMLDivElement; let root: Root;
const render = async () => act(async () => root.render(createElement(ExtensionsRegistry)));
const setValue = async (selector: string, value: string) => {
  const element = host.querySelector(selector) as HTMLInputElement | HTMLTextAreaElement;
  await act(async () => {
    const prototype = element instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(prototype, "value")!.set!.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
  });
};
const click = async (element: HTMLButtonElement | HTMLInputElement) => act(async () => { element.click(); await Promise.resolve(); });

beforeEach(() => {
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
  api.isNativeApp.mockReturnValue(true);
  api.extensionRegistry.mockResolvedValue([]);
  api.registerExtension.mockResolvedValue(registered);
  api.setExtensionPermissions.mockImplementation(async (update) => ({ ...registered, grantedCapabilities: update.grantedCapabilities }));
  api.setExtensionEnabled.mockImplementation(async (_id, enabled) => ({ ...registered, enabled }));
  api.removeExtension.mockResolvedValue(undefined);
});
afterEach(async () => { await act(async () => root.unmount()); host.remove(); vi.resetAllMocks(); });

describe("ExtensionsRegistry", () => {
  it("registers a typed manifest as a disabled metadata entry", async () => {
    await render();
    await setValue("#extension-id", manifest.id);
    await setValue("#extension-name", manifest.name);
    await setValue("#extension-version", manifest.version);
    await setValue("#extension-description", manifest.description);
    const capability = host.querySelectorAll<HTMLInputElement>('.extension-capability-picker input')[1];
    await click(capability);
    await click(host.querySelector<HTMLButtonElement>('button[type="submit"]')!);
    expect(api.registerExtension).toHaveBeenCalledWith(manifest);
    expect(host.textContent).toContain("was added as a disabled registry entry");
    expect(host.textContent).toContain("execution is unavailable");
    expect(host.querySelector<HTMLInputElement>(".extension-enable input")?.checked).toBe(false);
  });

  it("saves only a requested capability as a policy choice and clearly states it has no runtime effect", async () => {
    api.extensionRegistry.mockResolvedValue([registered]);
    await render();
    const grant = host.querySelector<HTMLInputElement>(".extension-grants input")!;
    expect(grant.checked).toBe(false);
    await click(grant);
    expect(api.setExtensionPermissions).toHaveBeenCalledWith({ extensionId: manifest.id, manifestVersion: manifest.version, grantedCapabilities: ["workspace_summary.read"] });
    expect(host.textContent).toContain("These grants have no effect until extension execution is implemented.");
  });

  it("keeps extensions disabled until an explicit enable action", async () => {
    api.extensionRegistry.mockResolvedValue([registered]);
    await render();
    const enabled = host.querySelector<HTMLInputElement>(".extension-enable input")!;
    expect(enabled.checked).toBe(false);
    await click(enabled);
    expect(api.setExtensionEnabled).toHaveBeenCalledWith(manifest.id, true);
    expect(host.textContent).toContain("Extension code is not executed by this app.");
  });

  it("asks for confirmation, returns focus to Keep, then removes the entry and saved grants", async () => {
    api.extensionRegistry.mockResolvedValue([registered]);
    await render();
    await click([...host.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent === "Remove")!);
    expect(api.removeExtension).not.toHaveBeenCalled();
    expect(document.activeElement?.textContent).toBe("Keep");
    await click([...host.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent === "Remove")!);
    expect(api.removeExtension).toHaveBeenCalledWith(manifest.id);
    expect(host.textContent).toContain("and its saved grants were removed");
  });

  it("shows browser limitations without attempting native calls", async () => {
    api.isNativeApp.mockReturnValue(false);
    await render();
    expect(host.textContent).toContain("native desktop app only");
    expect(api.extensionRegistry).not.toHaveBeenCalled();
  });

  it("surfaces backend loading errors", async () => {
    api.isNativeApp.mockReturnValue(true);
    api.extensionRegistry.mockRejectedValueOnce(new Error("Database unavailable"));
    await render();
    await act(async () => { await Promise.resolve(); await Promise.resolve(); });
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Database unavailable");
  });
});
