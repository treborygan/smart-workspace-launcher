import type { Resource } from "./contracts";
import type { SmartCollectionFilters } from "@sw/contracts";
export type { SmartCollection, SmartCollectionFilters } from "@sw/contracts";

/** Local AND-filter evaluation makes a saved collection react immediately to workspace updates. */
export function matchesSmartCollection(resource: Resource, filters: SmartCollectionFilters): boolean {
  if (filters.resourceType && resource.resourceType !== filters.resourceType) return false;
  if (filters.category && (resource.category || "").toLocaleLowerCase() !== filters.category.toLocaleLowerCase()) return false;
  if (filters.tag && !resource.tags.some((tag) => tag.toLocaleLowerCase() === filters.tag!.toLocaleLowerCase())) return false;
  if (filters.favoriteOnly && !resource.favorite) return false;
  const text = filters.text?.trim().toLocaleLowerCase();
  if (text) {
    const fields = [resource.displayName, resource.description, resource.category, ...resource.tags, ...resource.aliases, ...resource.targets.map((target) => target.canonicalUri)]
      .filter((value): value is string => Boolean(value)).map((value) => value.toLocaleLowerCase());
    if (!fields.some((value) => value.includes(text))) return false;
  }
  return true;
}

export function validateSmartCollection(name: string, filters: SmartCollectionFilters): string | null {
  if (!name.trim()) return "Collection name is required.";
  if ([...name.trim()].length > 100) return "Collection name must be 100 characters or fewer.";
  if (/[\u0000-\u001f\u007f-\u009f]/.test(name)) return "Collection name cannot contain control characters.";
  for (const [field, value, max] of [["Text filter", filters.text, 200], ["Category filter", filters.category, 80], ["Tag filter", filters.tag, 80]] as const) {
    if (value === undefined) continue;
    if (!value.trim()) return `${field} cannot contain only whitespace.`;
    if ([...value].length > max) return `${field} must be ${max} characters or fewer.`;
    if (/[\u0000-\u001f\u007f-\u009f]/.test(value)) return `${field} cannot contain control characters.`;
  }
  return null;
}
