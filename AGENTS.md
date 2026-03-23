# SkillBox Agent Notes

## Project Stack

- Frontend: Vue 3 + TypeScript + Pinia + Vite
- Desktop shell: Tauri v1
- Main UI entry: `src/App.vue`
- Global styles: `src/styles/main.css`

## Figma MCP

- The official remote Figma MCP server is already enabled globally in Codex as `figma`.
- Authentication should use Codex-managed OAuth, not project files or checked-in tokens.
- Do not store Figma personal access tokens, OAuth codes, or temporary keys in this repository.

## Working From Figma / Figma Make

- When the user shares a Figma file, frame, layer, or Make link, use the `figma` MCP server to fetch design context before editing code.
- Prefer implementing the selected screen or component inside the existing Vue/Tauri app rather than rebuilding a disconnected demo.
- Reuse the current structure in `src/App.vue` and `src/styles/main.css` unless the task clearly requires splitting components.
- Keep text, spacing, hierarchy, and interaction behavior aligned with the linked design, but adapt details as needed for desktop usability.

## Implementation Preferences

- Preserve the existing app's simple state flow unless the design introduces a clear need for more structure.
- Keep CSS variables centralized in `src/styles/main.css` when introducing new design tokens from Figma.
- If Figma Make provides generated code or resources, treat them as reference material and normalize them to this codebase's patterns before merging.
