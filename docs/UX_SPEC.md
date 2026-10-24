# UX and interaction spec

## Dashboard
12-column adaptive drag-and-drop grid with persistent layout positions, keyboard alternative to rearrangement, resizable widgets/card widths and heights, edit-mode undo/cancel/save, responsive overflow and focus retention. Side navigation for profiles/categories/favorites/analytics; Inspector panel on selection; search toolbar. Card variants: compact, standard, detail, group and widget. Configurable click/double-click action, context menu, size, icon, theme and content; never make destructive action default.

The app-wide close behavior is selectable: `Hide to tray` hides a launcher window and keeps the process and other windows available; `Exit` terminates the launcher process when a launcher window receives a close request. The default is `Hide to tray`. This preference is local to the app, independent of profiles, and is not included in profile exports.

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
