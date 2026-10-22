import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DevicePathMapping, DevicePathMappingPreview, LocalDeviceIdentity } from "@sw/contracts";
import { api } from "../lib/api";
import { DevicePathSettings } from "./DevicePathSettings";

vi.mock("../lib/api", () => ({ api: {
  localDevice: vi.fn(), renameLocalDevice: vi.fn(), devicePathMappings: vi.fn(),
  previewDevicePathMapping: vi.fn(), commitDevicePathMapping: vi.fn(), deleteDevicePathMapping: vi.fn(),
} }));

const device: LocalDeviceIdentity = { id: "8d302244-4bd7-49b8-894c-a21d895fca1a", name: "Studio PC" };
const mapping: DevicePathMapping = { id: "eaa7718b-2557-4d29-bac3-12d9e7caf255", deviceId: device.id, sourcePrefix: "C:\\Shared\\Plans", targetPrefix: "D:\\Work\\Plans", createdUtc: "2026-10-22T00:00:00Z" };
const preview: DevicePathMappingPreview = { deviceId: device.id, sourcePrefix: "C:\\Shared\\Plans", targetPrefix: "D:\\Work\\Plans", previewToken: "preview-token", changes: [{ resourceId: "res-1", targetId: "target-1", fromUri: "C:\\Shared\\Plans\\Q4.xlsx", toUri: "D:\\Work\\Plans\\Q4.xlsx" }] };
let host: HTMLDivElement; let root: Root;
const render = () => act(async () => root.render(createElement(DevicePathSettings)));
const setInput = async (selector: string, value: string) => {
  const input = host.querySelector(selector) as HTMLInputElement;
  await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, value); input.dispatchEvent(new Event("input", { bubbles: true })); });
};
const click = async (button: HTMLButtonElement) => act(async () => { button.click(); await Promise.resolve(); });

beforeEach(() => {
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
  vi.mocked(api.localDevice).mockResolvedValue(device);
  vi.mocked(api.devicePathMappings).mockResolvedValue([]);
  vi.mocked(api.renameLocalDevice).mockResolvedValue(device);
  vi.mocked(api.previewDevicePathMapping).mockResolvedValue(preview);
  vi.mocked(api.commitDevicePathMapping).mockResolvedValue(mapping);
  vi.mocked(api.deleteDevicePathMapping).mockResolvedValue(undefined);
});
afterEach(async () => { await act(async () => root.unmount()); host.remove(); vi.clearAllMocks(); });

describe("DevicePathSettings", () => {
  it("loads installation-local identity and shows approved rules without hiding their scope", async () => {
    vi.mocked(api.devicePathMappings).mockResolvedValue([mapping]);
    await render();
    expect(api.localDevice).toHaveBeenCalledOnce();
    expect(host.textContent).toContain("Local ID:");
    expect(host.textContent).toContain("Studio PC");
    expect(host.textContent).toContain("C:\\Shared\\Plans");
    expect(host.textContent).toContain("D:\\Work\\Plans");
    expect(host.textContent).toContain("Profile exports keep the original resource paths");
  });

  it("previews exact changes and requires explicit approval before saving", async () => {
    await render();
    await setInput("#mapping-source", preview.sourcePrefix);
    await setInput("#mapping-target", preview.targetPrefix);
    const previewButton = [...host.querySelectorAll("button")].find((button) => button.textContent === "Preview mapping")!;
    await click(previewButton);
    expect(api.previewDevicePathMapping).toHaveBeenCalledWith({ sourcePrefix: preview.sourcePrefix, targetPrefix: preview.targetPrefix });
    expect(host.textContent).toContain(preview.changes[0].fromUri);
    expect(host.textContent).toContain(preview.changes[0].toUri);
    expect(api.commitDevicePathMapping).not.toHaveBeenCalled();
    const approve = [...host.querySelectorAll("button")].find((button) => button.textContent === "Approve and save mapping")!;
    await click(approve);
    expect(api.commitDevicePathMapping).toHaveBeenCalledWith(preview);
    expect(host.textContent).toContain("Device path mapping approved and saved.");
  });

  it("clears a stale preview when paths are edited and after a stale commit fails", async () => {
    vi.mocked(api.commitDevicePathMapping).mockRejectedValue(new Error("This preview is stale. Create a fresh preview."));
    await render();
    await setInput("#mapping-source", preview.sourcePrefix); await setInput("#mapping-target", preview.targetPrefix);
    await click([...host.querySelectorAll("button")].find((button) => button.textContent === "Preview mapping")!);
    expect(host.textContent).toContain("Review before approval");
    await setInput("#mapping-target", "E:\\Projects");
    expect(host.textContent).not.toContain("Review before approval");
    await click([...host.querySelectorAll("button")].find((button) => button.textContent === "Preview mapping")!);
    await click([...host.querySelectorAll("button")].find((button) => button.textContent === "Approve and save mapping")!);
    expect(host.textContent).not.toContain("Review before approval");
    expect(host.textContent).toContain("This preview is stale");
  });

  it("requires a second explicit click to remove an approved mapping", async () => {
    vi.mocked(api.devicePathMappings).mockResolvedValue([mapping]);
    await render();
    await click(host.querySelector('[aria-label^="Remove mapping"]') as HTMLButtonElement);
    expect(api.deleteDevicePathMapping).not.toHaveBeenCalled();
    expect(host.textContent).toContain("Remove this rule?");
    await click([...host.querySelectorAll("button")].find((button) => button.textContent === "Remove")!);
    expect(api.deleteDevicePathMapping).toHaveBeenCalledWith(mapping.id);
    expect(host.textContent).toContain("Device path mapping removed.");
  });
});
