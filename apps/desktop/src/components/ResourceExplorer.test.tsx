import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Resource } from "../lib/contracts";
import { buildResourceCsv, ResourceExplorer } from "./ResourceExplorer";
const resources: Resource[] = [
  { id: "1", resourceType: "file", displayName: "Zeta", defaultAction: "open", createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z", targets: [{ id: "t1", kind: "file", canonicalUri: "C:\\Zeta.xlsx", priority: 0, arguments: [], active: true }], tags: ["finance"], aliases: [], favorite: false, launchCount: 3, category: "Work" },
  { id: "2", resourceType: "url", displayName: "Alpha", defaultAction: "open", createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z", targets: [{ id: "t2", kind: "url", canonicalUri: "https://alpha.example", priority: 0, arguments: [], active: true }], tags: [], aliases: [], favorite: true, launchCount: 8, category: "Web" },
];
let host: HTMLDivElement; let root: Root; const onSelect = vi.fn(); const onAdd = vi.fn(); const onUpdate = vi.fn();
beforeEach(() => { host = document.createElement("div"); document.body.append(host); root = createRoot(host); onSelect.mockReset(); onAdd.mockReset(); onUpdate.mockReset(); });
afterEach(async () => { await act(async () => root.unmount()); host.remove(); });
const render = async (items = resources) => act(async () => root.render(createElement(ResourceExplorer, { resources: items, onSelect, onAdd, onUpdate })));
const click = async (button: Element) => act(async () => { (button as HTMLButtonElement).click(); });

describe("ResourceExplorer", () => {
  it("sorts by a selected column and filters by type and collection", async () => {
    await render();
    expect([...host.querySelectorAll("tbody tr")].map((row) => row.textContent?.includes("Alpha") ? "Alpha" : "Zeta")).toEqual(["Alpha", "Zeta"]);
    const type = host.querySelector<HTMLSelectElement>('[aria-label="Filter by type"]')!;
    await act(async () => { type.value = "file"; type.dispatchEvent(new Event("change", { bubbles: true })); });
    expect(host.querySelectorAll("tbody tr")).toHaveLength(1);
    expect(host.textContent).toContain("Zeta");
    expect(host.textContent).not.toContain("Alpha");
    const allTypes = host.querySelector<HTMLSelectElement>('[aria-label="Filter by type"]')!;
    await act(async () => { allTypes.value = "all"; allTypes.dispatchEvent(new Event("change", { bubbles: true })); });
    const nameHeader = [...host.querySelectorAll("th button")].find((button) => button.textContent?.includes("Name"))!;
    await click(nameHeader);
    expect(host.querySelector("th[aria-sort='descending']")).not.toBeNull();
  });
  it("selects filtered rows and applies a non-destructive bulk favorite action", async () => {
    onUpdate.mockResolvedValue(true);
    await render();
    const checkbox = host.querySelector<HTMLInputElement>('[aria-label="Select Zeta"]')!;
    await act(async () => { checkbox.click(); });
    expect(host.textContent).toContain("1 selected");
    const action = [...host.querySelectorAll("button")].find((button) => button.textContent?.includes("Add selected to Favorites"))!;
    await click(action);
    expect(onUpdate).toHaveBeenCalledWith("1", { favorite: true });
    expect(host.textContent).toContain("1 of 1 selected resource added to Favorites.");
  });
  it("keeps failed bulk updates selected so the user can retry", async () => {
    onUpdate.mockResolvedValue(false);
    await render();
    const checkbox = host.querySelector<HTMLInputElement>('[aria-label="Select Zeta"]')!;
    await act(async () => { checkbox.click(); });
    const action = [...host.querySelectorAll("button")].find((button) => button.textContent?.includes("Add selected to Favorites"))!;
    await click(action);
    expect(host.textContent).toContain("failed items remain selected for retry");
    expect(host.textContent).toContain("1 selected");
  });
  it("exports quoted CSV and prevents formula execution", () => {
    const csv = buildResourceCsv([{ ...resources[0], displayName: "  =HYPERLINK(\"x\")", description: 'say "hi"' }], ["name", "location"]);
    expect(csv).toContain("\"Name\",\"Location\"");
    expect(csv).toContain('"\'  =HYPERLINK(""x"")"');
    expect(csv).toContain("C:\\Zeta.xlsx");
  });
  it("quotes commas, quotes and newlines in exported cells while protecting spaced formulas", () => {
    const csv = buildResourceCsv([{ ...resources[0], displayName: " \t+SUM(1,2)", category: 'Work, "Shared"\nTeam' }], ["name", "category"]);
    expect(csv).toContain('"\' \t+SUM(1,2)"');
    expect(csv).toContain('"Work, ""Shared""\nTeam"');
  });
  it("supports keyboard selection and column visibility", async () => {
    await render();
    const row = host.querySelector<HTMLTableRowElement>("tbody tr")!;
    await act(async () => { row.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })); });
    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: "2" }));
    const details = host.querySelector<HTMLDetailsElement>(".column-picker")!; details.open = true;
    const locationToggle = [...details.querySelectorAll<HTMLInputElement>("input")].find((input) => input.parentElement?.textContent === "Location")!;
    await act(async () => { locationToggle.click(); });
    expect(host.querySelector("th button")?.textContent).toContain("Name");
    expect(host.textContent).not.toContain("C:\\Zeta.xlsx");
  });
});
