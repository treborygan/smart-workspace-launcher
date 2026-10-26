import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { api } = vi.hoisted(() => ({ api: { getWindowCloseBehavior: vi.fn(), setWindowCloseBehavior: vi.fn() } }));
vi.mock("../lib/api", () => ({ api }));
import { WindowCloseBehaviorSettings } from "./WindowCloseBehaviorSettings";

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  api.getWindowCloseBehavior.mockReset();
  api.setWindowCloseBehavior.mockReset();
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
});

describe("window close behavior settings", () => {
  it("reads the app-wide preference and reflects it in accessible radios", async () => {
    api.getWindowCloseBehavior.mockResolvedValue("exit");
    await act(async () => { root.render(createElement(WindowCloseBehaviorSettings)); await Promise.resolve(); });

    expect(api.getWindowCloseBehavior).toHaveBeenCalledTimes(1);
    expect(host.querySelector<HTMLInputElement>('input[name="window-close-behavior"][value="exit"]')?.checked).toBe(true);
    expect(host.querySelector<HTMLInputElement>('input[name="window-close-behavior"][value="hide_to_tray"]')?.checked).toBe(false);
    expect(host.querySelector("fieldset legend")?.textContent).toBe("When closing a launcher window");
    expect(host.querySelector("fieldset")?.textContent).toContain("This app-wide choice applies to Dashboard, Quick Launch, and Dock close actions. Explicit Hide and Escape controls only hide the current window.");
  });

  it("saves a selected behavior and reflects the command result", async () => {
    api.getWindowCloseBehavior.mockResolvedValue("hide_to_tray");
    api.setWindowCloseBehavior.mockResolvedValue("exit");
    await act(async () => { root.render(createElement(WindowCloseBehaviorSettings)); await Promise.resolve(); });

    const exit = host.querySelector<HTMLInputElement>('input[name="window-close-behavior"][value="exit"]')!;
    await act(async () => { exit.click(); await Promise.resolve(); });

    expect(api.setWindowCloseBehavior).toHaveBeenCalledExactlyOnceWith("exit");
    expect(exit.checked).toBe(true);
    expect(host.querySelector<HTMLInputElement>('input[name="window-close-behavior"][value="hide_to_tray"]')?.checked).toBe(false);
  });

  it("shows load and save errors without claiming an unsaved choice", async () => {
    api.getWindowCloseBehavior.mockRejectedValueOnce(new Error("Preference read failed"));
    await act(async () => { root.render(createElement(WindowCloseBehaviorSettings)); await Promise.resolve(); });
    expect(host.querySelector('[role="alert"]')?.textContent).toBe("Preference read failed");

    await act(async () => root.unmount());
    api.getWindowCloseBehavior.mockResolvedValueOnce("hide_to_tray");
    api.setWindowCloseBehavior.mockRejectedValueOnce(new Error("Preference write failed"));
    root = createRoot(host);
    await act(async () => { root.render(createElement(WindowCloseBehaviorSettings)); await Promise.resolve(); });
    const exit = host.querySelector<HTMLInputElement>('input[name="window-close-behavior"][value="exit"]')!;
    await act(async () => { exit.click(); await Promise.resolve(); });

    expect(host.querySelector('[role="alert"]')?.textContent).toBe("Preference write failed");
    expect(host.querySelector<HTMLInputElement>('input[name="window-close-behavior"][value="hide_to_tray"]')?.checked).toBe(true);
    expect(exit.checked).toBe(false);
  });
});
