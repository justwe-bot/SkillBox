# SkillBox Agent Notes

## Project Overview

- SkillBox is a desktop tool for scanning, aggregating, linking, and syncing AI skill folders across multiple apps.
- Frontend stack: React 19 + TypeScript + Vite + React Router.
- Desktop shell: Tauri v1 with a Rust backend.
- Main UI entry: `src/App.tsx`
- Frontend bootstrap: `src/main.tsx`
- Global styles and design tokens: `src/styles/main.css`
- Main screens: `src/pages/DashboardPage.tsx` and `src/pages/SettingsPage.tsx`
- Tauri command layer: `src/lib/tauri.ts`
- Shared frontend types: `src/types.ts`
- Rust backend commands: `src-tauri/src/main.rs`
- Figma helper module: `src-tauri/src/figma.rs`

## Current App Behavior

- Routing uses `HashRouter`.
- The dashboard is the primary workspace for app discovery, skill inspection, Git sync, and link management.
- Settings covers local storage paths, custom scan paths, theme, and user preferences.
- User-facing copy is currently Chinese; preserve that tone unless a task explicitly asks for localization changes.
- Theme and lightweight preferences are handled client-side; avoid introducing heavier state management unless the task clearly needs it.

## Implementation Preferences

- Prefer extending the existing React/Tauri app instead of rebuilding isolated demos or parallel prototypes.
- Reuse the current page structure, modal patterns, toast flow, and shared components before introducing new abstractions.
- Keep CSS variables centralized in `src/styles/main.css` when adding tokens or adjusting the visual system.
- Preserve the existing `invoke` contracts in `src/lib/tauri.ts` unless a coordinated frontend and backend change is required.
- If backend payload shapes change, update both the Rust commands and the corresponding TypeScript types together.
- If documentation and code disagree, trust the live codebase first and update the docs as part of the change.

## Working From Figma / Figma Make

- When the user shares a Figma file, frame, layer, or Make link, use the `figma` MCP server to fetch design context before editing code.
- Implement designs inside the existing React/Tauri app whenever possible.
- Reuse `src/App.tsx`, the page components under `src/pages`, and `src/styles/main.css` unless the work clearly benefits from a new component split.
- Keep spacing, hierarchy, and interaction behavior aligned with the design, but adapt details for desktop usability and the current product language.
- Treat Figma Make output as reference material and normalize it to this codebase's patterns before merging.

## Figma Security

- The official remote Figma MCP server is available globally in Codex as `figma`.
- Authentication should use Codex-managed OAuth when working through MCP.
- Do not store Figma personal access tokens, OAuth codes, or temporary keys in this repository.
- If a task touches local app-side Figma configuration, keep secrets in user-level app config only and never commit them.

## Practical Editing Guidance

- Prefer small, targeted updates over broad refactors.
- Avoid changing unrelated files in a dirty worktree.
- Keep new user-facing actions consistent with the existing dashboard and settings flows.
- When adding new settings or commands, make sure the README and this file still reflect the real project structure.

## Release Process

### 版本号规则

- **主版本格式**：`YYYY.M.D`（如 `2026.3.26`）
- **当天后续更新**：`YYYY.M.D1`, `YYYY.M.D2`, `YYYY.M.D3`...（如 `2026.3.261`, `2026.3.262`）

示例：
- 2026年3月26日首次发布：`v2026.3.26`
- 2026年3月26日第二次发布（修复问题）：`v2026.3.261`
- 2026年3月26日第三次发布：`v2026.3.262`
- 2026年3月27日首次发布：`v2026.3.27`

注意：版本号必须符合 semver 格式（三个数字部分），Tauri 构建要求。

### 版本号文件

打 tag 发布新版本时，需要同步更新以下文件中的版本号：

1. **package.json** - 前端版本号
2. **src-tauri/tauri.conf.json** - Tauri 配置中的版本号
3. **src-tauri/Cargo.toml** - Rust 包版本号（这个会被 `get_version()` 读取）

### 发布流程

```bash
# 1. 更新版本号（修改上述3个文件）
# 2. 提交更改
git add -A && git commit -m "chore: bump version to x.x.x"

# 3. 打 tag 并推送（必须先更新版本号再打 tag，否则构建会使用旧版本）
git tag -a vx.x.x -m "Release version x.x.x"
git push origin main --tags
```

注意：打 tag 前必须先更新版本号并提交，CI 会基于 tag 时的代码自动构建发布。
