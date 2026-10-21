import { describe, expect, it } from "vitest";
import type { Resource } from "./contracts";
import { matchesSmartCollection, validateSmartCollection } from "./smartCollections";

const resource: Resource = { id: "r1", resourceType: "file", displayName: "Roadmap Q3", description: "Planning workbook", defaultAction: "open", createdUtc: "2026-01-01T00:00:00Z", updatedUtc: "2026-01-01T00:00:00Z", targets: [{ id: "t1", kind: "file", canonicalUri: "C:\\Work\\Roadmap.xlsx", priority: 0, arguments: [], active: true }], tags: ["planning", "finance"], aliases: ["plan"], favorite: true, launchCount: 3, category: "Work" };

describe("smart collection matching", () => {
  it("applies all configured fields with AND semantics to current resource values", () => {
    expect(matchesSmartCollection(resource, { resourceType: "file", category: "work", tag: "Planning", favoriteOnly: true, text: "roadmap" })).toBe(true);
    expect(matchesSmartCollection(resource, { category: "Web", text: "roadmap" })).toBe(false);
    expect(matchesSmartCollection(resource, { tag: "missing" })).toBe(false);
    expect(matchesSmartCollection({ ...resource, favorite: false }, { favoriteOnly: true })).toBe(false);
  });
  it("searches names, aliases, tags, descriptions and locations case-insensitively", () => {
    for (const text of ["plan", "finance", "workbook", "c:\\work"]) expect(matchesSmartCollection(resource, { text })).toBe(true);
  });
  it("validates user-facing contract length limits", () => {
    expect(validateSmartCollection(" ", {})).toContain("required");
    expect(validateSmartCollection("x".repeat(101), {})).toContain("100 characters");
    expect(validateSmartCollection("Good", { text: "x".repeat(201) })).toContain("200 characters");
    expect(validateSmartCollection("Good", { tag: "x".repeat(81) })).toContain("80 characters");
    expect(validateSmartCollection("Good", { text: "  " })).toContain("only whitespace");
    expect(validateSmartCollection("Good\nBad", {})).toContain("control characters");
    expect(validateSmartCollection("Good", { category: "Work\u0000Team" })).toContain("control characters");
    expect(validateSmartCollection("Good", {})).toBeNull();
  });
});
