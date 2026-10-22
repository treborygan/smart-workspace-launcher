# UX and interaction spec

## Dashboard
12-column adaptive drag-and-drop grid with persistent layout positions, keyboard alternative to rearrangement, resizable widgets/card widths and heights, edit-mode undo/cancel/save, responsive overflow and focus retention. Side navigation for profiles/categories/favorites/analytics; Inspector panel on selection; search toolbar. Card variants: compact, standard, detail, group and widget. Configurable click/double-click action, context menu, size, icon, theme and content; never make destructive action default.

## Quick Launch
Opens through configurable global hotkey, defaults suggested Alt+Space with conflict detection; should warm-open within 150ms goal. Focus search immediately, escape closes, arrow keys navigate, Enter launches, modifier/context action permits copy path, locate, edit and open-with. Filters include @profile, type:excel, #tag, >command, is:missing. Fuzzy ranking balanced across exact match, prefix, aliases, favorites, usage and recency; user may disable personalized ranking.

## Dock
Independent optional window; screen edge, orientation, monitor, icon size, spacing, opacity, always-on-top, autohide; shows per-profile pins and optional group. Tray and Dock survive Dashboard close if configured. Accurate DPI and multi-monitor handoff.

## Explorer/Inspector
Sortable/editable table with bulk actions, draggable columns, show/hide/resize/reorder, filters and CSV export. Inspector sections: targets, permissions, metadata, usage events, connections, profile overrides. Distinguish launcher launches from OS last-access values.

## Themes/accessibility
Light/dark/system, custom color variables, density presets, contrast and focus rings, keyboard-only accessible menus, reduced motion, 100–200% DPI. No platform-specific hardcoded file icons requiring privileged install.

## Empty/error states
Network unreachable, moved file, missing default handler, denied access, conflicting hotkey, bad import, unavailable WebView2: provide actionable message and keep UI responsive. No spinner that blocks Quick Launch waiting for a network path.

Workspace sequences are profile-local named lists of existing resources. Users can reorder up to 20 steps, select an optional target per step, and explicitly run the sequence. Show ordered progress and identify the failed step; stop there by default while preserving completed launch events. A retry of the same run request must not repeat launches. Imported sequence definitions are shown for review but never run during import.

SW-019 widget templates are selected from the built-in data views (resource shortcuts, recent launches, launch summary, related resources). Users can name reusable profile-local templates, place multiple instances in the dashboard grid, move/resize/remove instances, and edit or delete templates. Template options only reference existing profile resources or choose bounded display periods/limits. History widgets explain that they include only launches recorded by this application and reflect the profile's history preference. Empty/stale resource references render an actionable empty state and never trigger an action automatically.
