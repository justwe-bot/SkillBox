# SkillBox

SkillBox 是一个基于 Tauri 的桌面工具，用来统一扫描、整理、链接和同步多个 AI 应用里的技能目录。

## 功能特性

- 扫描常见 AI 应用的技能目录，并识别应用是否已安装、是否已链接
- 支持添加自定义扫描路径，纳入统一管理
- 聚合多个应用中的 skills 到一个本地同步目录
- 检测重复项和内容冲突，查看技能来源
- 支持技能重命名、删除、打开所在目录
- 支持为应用创建或取消软链接
- 提供 Git 仓库地址、用户名、分支配置，以及推送、拉取、同步操作
- 支持浅色、深色和跟随系统主题

## 技术栈

- React 19
- TypeScript
- Vite
- React Router
- Tauri v1
- Rust

## 环境要求

- Node.js 20+（建议）
- npm
- Rust stable toolchain
- Tauri v1 所需的本机构建环境

如果是首次配置 Tauri 环境，可以先参考 Tauri 官方文档安装对应平台依赖。

## 开发启动

安装依赖：

```bash
npm install
```

仅启动前端开发服务器：

```bash
npm run dev
```

启动完整桌面应用：

```bash
npm run tauri -- dev
```

## 构建

构建前端：

```bash
npm run build
```

构建桌面应用：

```bash
npm run tauri -- build
```

## 使用流程

1. 在设置页选择一个“技能存储目录”，作为本地统一同步目录。
2. 回到首页扫描应用，查看已发现的技能来源。
3. 如果某些应用不在预设列表中，可以添加自定义路径。
4. 使用“汇总技能”把各应用中的 skills 整理到本地同步目录。
5. 配置 Git 仓库地址、用户名和分支后，执行推送、拉取或同步。
6. 根据需要为支持的应用创建或取消软链接。

## 项目结构

```text
skillbox/
├─ src/
│  ├─ App.tsx                 # 路由入口
│  ├─ main.tsx                # 前端启动入口
│  ├─ pages/                  # Dashboard / Settings 页面
│  ├─ components/             # 复用 UI 组件
│  ├─ lib/                    # Tauri 调用、主题、偏好设置
│  ├─ styles/main.css         # 全局样式与设计变量
│  └─ types.ts                # 前端类型定义
├─ src-tauri/
│  ├─ src/main.rs             # Tauri 命令与系统集成
│  ├─ src/figma.rs            # Figma 相关辅助逻辑
│  ├─ Cargo.toml
│  └─ tauri.conf.json
├─ AGENTS.md                  # 给 AI / 协作者的仓库协作说明
└─ package.json
```

## 代码约定

- 现有界面文案以中文为主，新增功能默认保持一致。
- 前端通过 `src/lib/tauri.ts` 调用 Tauri 命令，新增能力时请同步更新 Rust 端和 TypeScript 类型。
- 全局视觉变量优先放在 `src/styles/main.css`，避免把设计 token 分散到多个文件。

## Figma 与设计实现

- 仓库协作默认优先使用 Codex 中已启用的 `figma` MCP 服务获取设计上下文。
- 不要把 Figma 的个人令牌、OAuth 临时凭据或其他密钥提交到仓库。
- 如果从 Figma Make 或设计稿落地界面，优先在现有 React/Tauri 结构中实现，而不是单独新建 demo。

## 说明

当前仓库的实际实现是 React + Tauri；如果你看到旧文档中提到 Vue，请以当前代码结构为准。
