#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod figma;

use figma::{
    extract_css_from_node, extract_design_tokens, find_nodes_by_name, find_nodes_by_type,
    DesignToken, FigmaClient, FigmaComment, FigmaFile, FigmaFileData,
};
use futures_util::StreamExt;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{BufReader, Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::Manager;
use tokio::fs as tokio_fs;
use tokio::io::AsyncWriteExt;
use walkdir::{DirEntry, WalkDir};
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SkillApp {
    id: String,
    name: String,
    path: String,
    icon: String,
    skill_count: usize,
    enabled_skill_count: usize,
    is_linked: bool,
    is_installed: bool,
    is_custom: bool,
    backup_path: Option<String>,
    custom_path: Option<String>,
    link_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SkillFile {
    name: String,
    path: String,
    size: u64,
    modified: String,
    description: String,
    canonical_name: String,
    content_hash: String,
    file_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MarketSkillRecord {
    package_id: String,
    repository: String,
    name: String,
    author: String,
    downloads_label: String,
    url: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rating_label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MarketSkillDetail {
    package_id: String,
    readme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    #[serde(default)]
    git_path: Option<String>,
    #[serde(default = "default_git_config")]
    git_config: GitSyncConfig,
    #[serde(default)]
    custom_paths: HashMap<String, String>,
    #[serde(default)]
    enabled_skills_by_app: HashMap<String, Vec<String>>,
    #[serde(default)]
    figma_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedSkillEntry {
    entry_name: String,
    name: String,
    path: String,
    size: u64,
    modified: String,
    description: String,
    canonical_name: String,
    content_hash: String,
    file_count: usize,
    enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppEnabledSkillsResponse {
    app_id: String,
    link_mode: String,
    enabled_entries: Vec<String>,
    skills: Vec<ManagedSkillEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncEnabledSkillsConfig {
    #[serde(default)]
    enabled_skills_by_app: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillLockEntry {
    source_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_app_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_path: Option<String>,
    canonical_name: String,
    content_hash: String,
    file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillLockFile {
    version: u32,
    #[serde(default)]
    skills: BTreeMap<String, SkillLockEntry>,
}

impl Default for SkillLockFile {
    fn default() -> Self {
        Self {
            version: 1,
            skills: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitSyncConfig {
    #[serde(default)]
    repo_url: String,
    #[serde(default = "default_git_branch")]
    branch: String,
}

#[derive(Debug, Clone, Copy)]
struct SyncWorkspaceOptions {
    remove_missing_entries: bool,
    skip_source_local_only: bool,
    preserve_target_local_only: bool,
}

#[derive(Debug, Clone)]
struct KnownApp {
    id: String,
    name: String,
    icon: String,
    skill_paths: Vec<PathBuf>,
    install_markers: Vec<PathBuf>,
}

#[derive(Debug)]
struct ParsedSkillMetadata {
    name: String,
    description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCheckResult {
    current_version: String,
    latest_version: Option<String>,
    update_available: bool,
    release_url: String,
    release_name: Option<String>,
    published_at: Option<String>,
    notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadUpdateResult {
    version: String,
    file_name: String,
    file_path: String,
    release_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateDownloadProgressPayload {
    file_name: String,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    percentage: f64,
    status: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    name: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GitHubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubRepositoryInfo {
    default_branch: String,
}

const SYNC_MANIFEST_FILE: &str = ".skillbox-sync.json";
const SYNC_ENABLED_SKILLS_FILE: &str = ".skillbox-enabled-skills.json";
const SKILL_LOCK_FILE: &str = "skills-lock.json";
const INTERNAL_GIT_REPO_DIR: &str = ".skillbox-git";
const GITHUB_REPOSITORY: &str = "justwe-bot/SkillBox";
const GITHUB_REPOSITORY_URL: &str = "https://github.com/justwe-bot/SkillBox";
const UPDATE_DOWNLOAD_PROGRESS_EVENT: &str = "skillbox://update-download-progress";
const MARKET_TRENDING_URL: &str = "https://skills.sh/trending";
const MAX_RECOMMENDED_MARKET_SKILLS: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanPlatform {
    MacOs,
    Windows,
    Linux,
}

impl ScanPlatform {
    fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

fn known_app(
    id: &str,
    name: &str,
    icon: &str,
    skill_paths: Vec<PathBuf>,
    install_markers: Vec<PathBuf>,
) -> KnownApp {
    KnownApp {
        id: id.to_string(),
        name: name.to_string(),
        icon: icon.to_string(),
        skill_paths,
        install_markers,
    }
}

fn build_known_apps() -> Vec<KnownApp> {
    let home = dirs::home_dir().unwrap_or_default();

    match ScanPlatform::current() {
        ScanPlatform::MacOs => build_macos_known_apps(&home),
        ScanPlatform::Windows => build_windows_known_apps(&home),
        ScanPlatform::Linux => build_linux_known_apps(&home),
    }
}

fn build_macos_known_apps(home: &Path) -> Vec<KnownApp> {
    let app_support = home.join("Library/Application Support");

    vec![
        known_app(
            "codex",
            "Codex",
            "📦",
            vec![home.join(".codex/skills")],
            vec![
                PathBuf::from("/Applications/Codex.app"),
                home.join(".codex"),
                home.join(".codex/config.toml"),
            ],
        ),
        known_app(
            "openclaw",
            "Openclaw",
            "🦀",
            vec![
                home.join(".openclaw/workspace/skills"),
                home.join(".openclaw/skills"),
            ],
            vec![home.join(".openclaw")],
        ),
        known_app(
            "opencode",
            "Opencode",
            "💻",
            vec![home.join(".config/opencode/skills")],
            vec![home.join(".config/opencode")],
        ),
        known_app(
            "cline",
            "Cline",
            "⚡",
            vec![home.join(".cline/skills"), home.join(".cline/rules")],
            vec![home.join(".cline")],
        ),
        known_app(
            "cursor",
            "Cursor",
            "🎯",
            vec![
                home.join(".cursor/rules"),
                home.join(".cursor/skills"),
                app_support.join("Cursor/User/globalStorage/skills"),
            ],
            vec![
                PathBuf::from("/Applications/Cursor.app"),
                app_support.join("Cursor"),
                home.join(".cursor"),
            ],
        ),
        known_app(
            "windsurf",
            "Windsurf",
            "🌊",
            vec![
                home.join(".windsurf/rules"),
                home.join(".windsurf/skills"),
                app_support.join("Codeium/windsurf/memories"),
                app_support.join("Windsurf/User/globalStorage/skills"),
            ],
            vec![
                PathBuf::from("/Applications/Windsurf.app"),
                app_support.join("Windsurf"),
                app_support.join("Codeium"),
                home.join(".windsurf"),
            ],
        ),
        known_app(
            "trae",
            "Trae",
            "🧭",
            vec![
                home.join(".trae/skills"),
                home.join(".trae-cn/skills"),
                home.join(".trae/rules"),
                home.join(".trae-cn/rules"),
                app_support.join("Trae/User/globalStorage"),
                app_support.join("Trae CN/User/globalStorage"),
            ],
            vec![
                PathBuf::from("/Applications/Trae.app"),
                PathBuf::from("/Applications/Trae CN.app"),
                app_support.join("Trae"),
                app_support.join("Trae CN"),
                home.join(".trae"),
                home.join(".trae-cn"),
            ],
        ),
        known_app(
            "kiro",
            "Kiro",
            "🪄",
            vec![
                home.join(".kiro/skills"),
                home.join(".kiro/steering"),
                home.join(".kiro/powers"),
            ],
            vec![
                PathBuf::from("/Applications/Kiro.app"),
                app_support.join("Kiro"),
                home.join(".kiro"),
            ],
        ),
        known_app(
            "qoder",
            "Qoder",
            "🧩",
            vec![
                home.join(".qoder/commands"),
                home.join(".qoder/agents"),
                home.join(".qoder/rules"),
            ],
            vec![
                PathBuf::from("/Applications/Qoder.app"),
                app_support.join("Qoder"),
                home.join(".qoder"),
            ],
        ),
        known_app(
            "codebuddy",
            "CodeBuddy",
            "🤝",
            vec![
                home.join(".codebuddy/skills"),
                home.join(".codebuddy/prompts"),
                home.join(".codebuddycn/skills"),
                home.join(".codebuddycn/prompts"),
            ],
            vec![
                PathBuf::from("/Applications/CodeBuddy.app"),
                PathBuf::from("/Applications/CodeBuddy CN.app"),
                app_support.join("CodeBuddy"),
                app_support.join("CodeBuddy CN"),
                app_support.join("CodeBuddyExtension"),
                home.join(".codebuddy"),
                home.join(".codebuddycn"),
                home.join("CodeBuddy"),
            ],
        ),
        known_app(
            "copilot",
            "GitHub Copilot",
            "🧠",
            vec![home.join(".copilot/skills")],
            vec![home.join(".copilot"), app_support.join("GitHub Copilot")],
        ),
        known_app(
            "claude",
            "Claude",
            "🤖",
            vec![
                home.join(".claude/skills"),
                app_support.join("Claude/claude_desktop_skills"),
            ],
            vec![
                PathBuf::from("/Applications/Claude.app"),
                app_support.join("Claude"),
                home.join(".claude"),
            ],
        ),
        known_app(
            "roocode",
            "RooCode",
            "🦘",
            vec![home.join(".roo/skills")],
            vec![home.join(".roo")],
        ),
        known_app(
            "gemini",
            "Gemini CLI",
            "✨",
            vec![
                home.join(".gemini/commands"),
                home.join(".gemini/GEMINI.md"),
            ],
            vec![home.join(".gemini"), home.join(".gemini/settings.json")],
        ),
    ]
}

fn build_windows_known_apps(home: &Path) -> Vec<KnownApp> {
    let app_data = PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default());

    vec![
        known_app(
            "codex",
            "Codex",
            "📦",
            vec![home.join(".codex/skills")],
            vec![home.join(".codex"), home.join(".codex/config.toml")],
        ),
        known_app(
            "openclaw",
            "Openclaw",
            "🦀",
            vec![
                home.join(".openclaw/workspace/skills"),
                home.join(".openclaw/skills"),
            ],
            vec![home.join(".openclaw")],
        ),
        known_app(
            "opencode",
            "Opencode",
            "💻",
            vec![home.join(".config/opencode/skills")],
            vec![home.join(".config/opencode")],
        ),
        known_app(
            "cline",
            "Cline",
            "⚡",
            vec![home.join(".cline/skills"), home.join(".cline/rules")],
            vec![home.join(".cline")],
        ),
        known_app(
            "cursor",
            "Cursor",
            "🎯",
            vec![
                home.join(".cursor/rules"),
                home.join(".cursor/skills"),
                app_data.join("Cursor/User/globalStorage/skills"),
            ],
            vec![app_data.join("Cursor"), home.join(".cursor")],
        ),
        known_app(
            "windsurf",
            "Windsurf",
            "🌊",
            vec![
                home.join(".windsurf/rules"),
                home.join(".windsurf/skills"),
                app_data.join("Codeium/windsurf/memories"),
                app_data.join("Windsurf/User/globalStorage/skills"),
            ],
            vec![
                app_data.join("Windsurf"),
                app_data.join("Codeium"),
                home.join(".windsurf"),
            ],
        ),
        known_app(
            "trae",
            "Trae",
            "🧭",
            vec![
                home.join(".trae/skills"),
                home.join(".trae-cn/skills"),
                home.join(".trae/rules"),
                home.join(".trae-cn/rules"),
                app_data.join("Trae/User/globalStorage"),
                app_data.join("Trae CN/User/globalStorage"),
            ],
            vec![
                app_data.join("Trae"),
                app_data.join("Trae CN"),
                home.join(".trae"),
                home.join(".trae-cn"),
            ],
        ),
        known_app(
            "kiro",
            "Kiro",
            "🪄",
            vec![
                home.join(".kiro/skills"),
                home.join(".kiro/steering"),
                home.join(".kiro/powers"),
            ],
            vec![app_data.join("Kiro"), home.join(".kiro")],
        ),
        known_app(
            "qoder",
            "Qoder",
            "🧩",
            vec![
                home.join(".qoder/commands"),
                home.join(".qoder/agents"),
                home.join(".qoder/rules"),
            ],
            vec![app_data.join("Qoder"), home.join(".qoder")],
        ),
        known_app(
            "codebuddy",
            "CodeBuddy",
            "🤝",
            vec![
                home.join(".codebuddy/skills"),
                home.join(".codebuddy/prompts"),
                home.join(".codebuddycn/skills"),
                home.join(".codebuddycn/prompts"),
            ],
            vec![
                app_data.join("CodeBuddy"),
                app_data.join("CodeBuddy CN"),
                home.join(".codebuddy"),
                home.join(".codebuddycn"),
                home.join("CodeBuddy"),
            ],
        ),
        known_app(
            "copilot",
            "GitHub Copilot",
            "🧠",
            vec![home.join(".copilot/skills")],
            vec![home.join(".copilot"), app_data.join("GitHub Copilot")],
        ),
        known_app(
            "claude",
            "Claude",
            "🤖",
            vec![
                home.join(".claude/skills"),
                app_data.join("Claude/claude_desktop_skills"),
            ],
            vec![app_data.join("Claude"), home.join(".claude")],
        ),
        known_app(
            "roocode",
            "RooCode",
            "🦘",
            vec![home.join(".roo/skills")],
            vec![home.join(".roo")],
        ),
        known_app(
            "gemini",
            "Gemini CLI",
            "✨",
            vec![
                home.join(".gemini/commands"),
                home.join(".gemini/GEMINI.md"),
            ],
            vec![home.join(".gemini"), home.join(".gemini/settings.json")],
        ),
    ]
}

fn build_linux_known_apps(home: &Path) -> Vec<KnownApp> {
    let config_dir = home.join(".config");

    vec![
        known_app(
            "codex",
            "Codex",
            "📦",
            vec![home.join(".codex/skills")],
            vec![home.join(".codex"), home.join(".codex/config.toml")],
        ),
        known_app(
            "openclaw",
            "Openclaw",
            "🦀",
            vec![
                home.join(".openclaw/workspace/skills"),
                home.join(".openclaw/skills"),
            ],
            vec![home.join(".openclaw")],
        ),
        known_app(
            "opencode",
            "Opencode",
            "💻",
            vec![config_dir.join("opencode/skills")],
            vec![config_dir.join("opencode")],
        ),
        known_app(
            "cline",
            "Cline",
            "⚡",
            vec![home.join(".cline/skills"), home.join(".cline/rules")],
            vec![home.join(".cline")],
        ),
        known_app(
            "cursor",
            "Cursor",
            "🎯",
            vec![
                home.join(".cursor/rules"),
                home.join(".cursor/skills"),
                config_dir.join("Cursor/User/globalStorage/skills"),
            ],
            vec![config_dir.join("Cursor"), home.join(".cursor")],
        ),
        known_app(
            "windsurf",
            "Windsurf",
            "🌊",
            vec![
                home.join(".windsurf/rules"),
                home.join(".windsurf/skills"),
                config_dir.join("Codeium/windsurf/memories"),
                config_dir.join("Windsurf/User/globalStorage/skills"),
            ],
            vec![
                config_dir.join("Windsurf"),
                config_dir.join("Codeium"),
                home.join(".windsurf"),
            ],
        ),
        known_app(
            "trae",
            "Trae",
            "🧭",
            vec![
                home.join(".trae/skills"),
                home.join(".trae-cn/skills"),
                home.join(".trae/rules"),
                home.join(".trae-cn/rules"),
                config_dir.join("Trae/User/globalStorage"),
                config_dir.join("Trae CN/User/globalStorage"),
            ],
            vec![
                config_dir.join("Trae"),
                config_dir.join("Trae CN"),
                home.join(".trae"),
                home.join(".trae-cn"),
            ],
        ),
        known_app(
            "kiro",
            "Kiro",
            "🪄",
            vec![
                home.join(".kiro/skills"),
                home.join(".kiro/steering"),
                home.join(".kiro/powers"),
            ],
            vec![config_dir.join("Kiro"), home.join(".kiro")],
        ),
        known_app(
            "qoder",
            "Qoder",
            "🧩",
            vec![
                home.join(".qoder/commands"),
                home.join(".qoder/agents"),
                home.join(".qoder/rules"),
            ],
            vec![config_dir.join("Qoder"), home.join(".qoder")],
        ),
        known_app(
            "codebuddy",
            "CodeBuddy",
            "🤝",
            vec![
                home.join(".codebuddy/skills"),
                home.join(".codebuddy/prompts"),
                home.join(".codebuddycn/skills"),
                home.join(".codebuddycn/prompts"),
            ],
            vec![
                config_dir.join("CodeBuddy"),
                config_dir.join("CodeBuddy CN"),
                home.join(".codebuddy"),
                home.join(".codebuddycn"),
                home.join("CodeBuddy"),
            ],
        ),
        known_app(
            "copilot",
            "GitHub Copilot",
            "🧠",
            vec![home.join(".copilot/skills")],
            vec![home.join(".copilot"), config_dir.join("GitHub Copilot")],
        ),
        known_app(
            "claude",
            "Claude",
            "🤖",
            vec![
                home.join(".claude/skills"),
                config_dir.join("claude/claude_desktop_skills"),
            ],
            vec![config_dir.join("claude"), home.join(".claude")],
        ),
        known_app(
            "roocode",
            "RooCode",
            "🦘",
            vec![home.join(".roo/skills")],
            vec![home.join(".roo")],
        ),
        known_app(
            "gemini",
            "Gemini CLI",
            "✨",
            vec![
                home.join(".gemini/commands"),
                home.join(".gemini/GEMINI.md"),
            ],
            vec![home.join(".gemini"), home.join(".gemini/settings.json")],
        ),
    ]
}

fn find_known_app(app_id: &str) -> Option<KnownApp> {
    build_known_apps().into_iter().find(|app| app.id == app_id)
}

fn should_skip_walk_entry(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }

    let name = entry.file_name().to_string_lossy();

    // Skip all hidden directories/files (starting with .)
    if name.starts_with('.') {
        return false;
    }

    // Always skip these directories (at any depth)
    if matches!(name.as_ref(), "node_modules" | "target" | "__pycache__") {
        return false;
    }

    true
}

fn is_instruction_markdown(file_name: &str, parent_name: Option<&str>) -> bool {
    let lower_name = file_name.to_ascii_lowercase();
    if matches!(
        lower_name.as_str(),
        "skill.md" | "agents.md" | "claude.md" | "gemini.md" | "copilot-instructions.md"
    ) {
        return true;
    }

    if lower_name.ends_with(".instructions.md") || lower_name.ends_with(".prompt.md") {
        return true;
    }

    let parent = parent_name.unwrap_or_default().to_ascii_lowercase();
    lower_name.ends_with(".md")
        && matches!(
            parent.as_str(),
            "rules" | "prompts" | "checks" | "instructions" | "memories"
        )
}

fn is_supported_instruction_file(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let parent_name = path
        .parent()
        .and_then(|value| value.file_name())
        .and_then(|value| value.to_str());

    if is_instruction_markdown(file_name, parent_name) {
        return true;
    }

    match path.extension().and_then(|value| value.to_str()) {
        Some("skill") | Some("mdc") => true,
        Some("toml") => matches!(parent_name, Some("commands")),
        _ => false,
    }
}

fn normalize_skill_name(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_separator = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            normalized.push('-');
            previous_was_separator = true;
        }
    }

    normalized.trim_matches('-').to_string()
}

fn format_system_time(time: std::time::SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Utc> = time.into();
    datetime.format("%Y-%m-%d %H:%M").to_string()
}

fn parse_skill_metadata(content: &str, fallback_name: &str) -> ParsedSkillMetadata {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut name = None;
    let mut description = None;
    let mut body_lines = Vec::new();

    let mut lines = content.lines();
    let in_frontmatter = matches!(lines.next(), Some(line) if line.trim() == "---");

    if in_frontmatter {
        let mut frontmatter_lines = Vec::new();
        for line in &mut lines {
            if line.trim() == "---" {
                break;
            }
            frontmatter_lines.push(line.to_string());
        }

        let mut index = 0usize;
        while index < frontmatter_lines.len() {
            let line = &frontmatter_lines[index];

            if let Some((key, raw_value)) = line.split_once(':') {
                let key = key.trim();
                let value = raw_value.trim().trim_matches('"').trim_matches('\'');

                match key {
                    "name" if !value.is_empty() && value != "|" && value != ">" => {
                        name = Some(value.to_string());
                    }
                    "description" if !value.is_empty() && value != "|" && value != ">" => {
                        description = Some(value.to_string());
                    }
                    "description" if value == "|" || value == ">" => {
                        let mut block_lines = Vec::new();
                        let mut next_index = index + 1;

                        while next_index < frontmatter_lines.len() {
                            let next_line = &frontmatter_lines[next_index];
                            if next_line.trim().is_empty() {
                                next_index += 1;
                                continue;
                            }

                            let is_indented = next_line
                                .chars()
                                .next()
                                .map(|ch| ch.is_whitespace())
                                .unwrap_or(false);

                            if !is_indented {
                                break;
                            }

                            block_lines.push(next_line.trim().to_string());
                            next_index += 1;
                        }

                        let normalized = block_lines.join(" ").trim().to_string();
                        if !normalized.is_empty() {
                            description = Some(normalized);
                        }

                        index = next_index;
                        continue;
                    }
                    _ => {}
                }
            }

            index += 1;
        }
    }

    if name.is_none() || description.is_none() {
        for line in content.lines() {
            if let Some((key, value)) = line.split_once('=') {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                match key.trim() {
                    "name" if name.is_none() && !value.is_empty() => {
                        name = Some(value.to_string());
                    }
                    "description" if description.is_none() && !value.is_empty() => {
                        description = Some(value.to_string());
                    }
                    _ => {}
                }
            }
        }
    }

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with("```")
            || trimmed.starts_with("- ")
        {
            continue;
        }

        body_lines.push(trimmed.to_string());
        if body_lines.len() >= 2 {
            break;
        }
    }

    ParsedSkillMetadata {
        name: name.unwrap_or_else(|| fallback_name.to_string()),
        description: description.unwrap_or_else(|| body_lines.join(" ").trim().to_string()),
    }
}

fn inspect_skill_target(path: &Path) -> Result<(u64, String, usize, String), String> {
    let mut total_size = 0u64;
    let mut file_count = 0usize;
    let mut latest_modified: Option<std::time::SystemTime> = None;
    let mut hasher = Sha256::new();

    if path.is_file() {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
        total_size = metadata.len();
        file_count = 1;
        latest_modified = metadata.modified().ok();
        hasher.update(
            path.file_name()
                .and_then(|v| v.to_str())
                .unwrap_or_default()
                .as_bytes(),
        );
        hasher.update(&bytes);
    } else {
        for entry in WalkDir::new(path)
            .sort_by_file_name()
            .follow_links(true)
            .into_iter()
            .filter_entry(should_skip_walk_entry)
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };

            let bytes = match fs::read(entry.path()) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };

            let relative = entry
                .path()
                .strip_prefix(path)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .to_string();

            total_size += metadata.len();
            file_count += 1;
            latest_modified = match (latest_modified, metadata.modified().ok()) {
                (Some(current), Some(candidate)) if candidate > current => Some(candidate),
                (None, candidate) => candidate,
                (current, _) => current,
            };

            hasher.update(relative.as_bytes());
            hasher.update(&bytes);
        }
    }

    let modified = latest_modified.map(format_system_time).unwrap_or_default();

    Ok((
        total_size,
        modified,
        file_count,
        format!("{:x}", hasher.finalize()),
    ))
}

fn collect_skill_entries(path: &Path) -> Result<Vec<SkillFile>, String> {
    if !path.exists() {
        return Ok(vec![]);
    }

    let mut seen_paths = HashSet::new();
    let mut skills = Vec::new();

    if path.is_file() {
        if !is_supported_instruction_file(path) {
            return Ok(vec![]);
        }

        let fallback_name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("unknown")
            .to_string();
        let content = fs::read_to_string(path).unwrap_or_default();
        let metadata = parse_skill_metadata(&content, &fallback_name);
        let canonical_name = normalize_skill_name(&metadata.name);
        let (size, modified, file_count, content_hash) = inspect_skill_target(path)?;

        return Ok(vec![SkillFile {
            name: metadata.name,
            path: path.to_string_lossy().to_string(),
            size,
            modified,
            description: metadata.description,
            canonical_name: if canonical_name.is_empty() {
                normalize_skill_name(&fallback_name)
            } else {
                canonical_name
            },
            content_hash,
            file_count,
        }]);
    }

    for entry in WalkDir::new(path)
        .sort_by_file_name()
        .follow_links(true)
        .into_iter()
        .filter_entry(should_skip_walk_entry)
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let file_name = entry.file_name().to_string_lossy().to_string();
        let is_skill_markdown = file_name.eq_ignore_ascii_case("SKILL.md");

        if !is_supported_instruction_file(entry.path()) {
            continue;
        }

        let skill_path = if is_skill_markdown {
            entry.path().parent().unwrap_or(path).to_path_buf()
        } else {
            entry.path().to_path_buf()
        };

        let skill_key = skill_path.to_string_lossy().to_string();
        if !seen_paths.insert(skill_key) {
            continue;
        }

        let fallback_name = if is_skill_markdown {
            skill_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("unknown")
                .to_string()
        } else {
            entry
                .path()
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("unknown")
                .to_string()
        };

        let content = fs::read_to_string(entry.path()).unwrap_or_default();
        let metadata = parse_skill_metadata(&content, &fallback_name);
        let canonical_name = normalize_skill_name(&metadata.name);
        let (size, modified, file_count, content_hash) = inspect_skill_target(&skill_path)?;

        skills.push(SkillFile {
            name: metadata.name,
            path: skill_path.to_string_lossy().to_string(),
            size,
            modified,
            description: metadata.description,
            canonical_name: if canonical_name.is_empty() {
                normalize_skill_name(&fallback_name)
            } else {
                canonical_name
            },
            content_hash,
            file_count,
        });
    }

    skills.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(skills)
}

fn resolve_skill_path(app_id: &str, config: &AppConfig) -> Result<String, String> {
    if let Some(path) = config.custom_paths.get(app_id) {
        let custom = PathBuf::from(path);
        if app_id == "openclaw" {
            let workspace_root = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".openclaw/workspace");
            let openclaw_root = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".openclaw");

            if paths_match(&custom, &workspace_root) || custom == workspace_root {
                return Ok(workspace_root.join("skills").to_string_lossy().to_string());
            }

            if paths_match(&custom, &openclaw_root) || custom == openclaw_root {
                return Ok(openclaw_root
                    .join("workspace/skills")
                    .to_string_lossy()
                    .to_string());
            }
        }

        return Ok(path.clone());
    }

    let app = find_known_app(app_id).ok_or_else(|| format!("App {} not found", app_id))?;
    let resolved = app
        .skill_paths
        .iter()
        .find(|path| path.exists())
        .cloned()
        .unwrap_or_else(|| app.skill_paths.first().cloned().unwrap_or_default());

    Ok(resolved.to_string_lossy().to_string())
}

fn get_legacy_skill_paths(app_id: &str) -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    match app_id {
        "copilot" => vec![
            home.join(".copilot/copilot-instructions.md"),
            home.join(".github/copilot-instructions.md"),
            home.join(".github/instructions"),
        ],
        "openclaw" => vec![home.join(".openclaw/skills")],
        _ => Vec::new(),
    }
}

fn get_config_path() -> PathBuf {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    config_dir.join("skillbox")
}

fn default_git_branch() -> String {
    "main".to_string()
}

fn default_git_config() -> GitSyncConfig {
    GitSyncConfig {
        repo_url: String::new(),
        branch: default_git_branch(),
    }
}

fn load_config() -> AppConfig {
    let config_path = get_config_path().join("config.json");
    if config_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                return config;
            }
        }
    }
    AppConfig {
        git_path: None,
        git_config: default_git_config(),
        custom_paths: HashMap::new(),
        enabled_skills_by_app: HashMap::new(),
        figma_api_key: None,
    }
}

fn save_config(config: &AppConfig) -> Result<(), String> {
    let config_dir = get_config_path();
    fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    let config_path = config_dir.join("config.json");
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&config_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

fn get_sync_enabled_skills_path(sync_dir: &Path) -> PathBuf {
    sync_dir.join(SYNC_ENABLED_SKILLS_FILE)
}

fn get_skill_lock_path(sync_dir: &Path) -> PathBuf {
    sync_dir.join(SKILL_LOCK_FILE)
}

fn load_skill_lock(sync_dir: &Path) -> Result<SkillLockFile, String> {
    let lock_path = get_skill_lock_path(sync_dir);
    if !lock_path.exists() {
        return Ok(SkillLockFile::default());
    }

    let content = fs::read_to_string(&lock_path).map_err(|e| e.to_string())?;
    if let Ok(lock) = serde_json::from_str::<SkillLockFile>(&content) {
        return Ok(lock);
    }

    if let Ok(skills) = serde_json::from_str::<BTreeMap<String, SkillLockEntry>>(&content) {
        return Ok(SkillLockFile { version: 1, skills });
    }

    Err(format!(
        "无法解析同步目录中的技能锁文件: {}",
        lock_path.to_string_lossy()
    ))
}

fn save_skill_lock(sync_dir: &Path, lock_file: &SkillLockFile) -> Result<(), String> {
    fs::create_dir_all(sync_dir).map_err(|e| e.to_string())?;
    let lock_path = get_skill_lock_path(sync_dir);
    let content = serde_json::to_string_pretty(lock_file).map_err(|e| e.to_string())?;
    fs::write(&lock_path, format!("{}\n", content)).map_err(|e| e.to_string())
}

fn resolve_sync_entry_name_for_path(sync_dir: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(sync_dir).ok()?;
    sanitize_sync_entry_name(&relative.to_string_lossy()).ok()
}

fn rename_skill_lock_entry(
    sync_dir: &Path,
    old_path: &Path,
    new_path: &Path,
) -> Result<(), String> {
    let Some(old_entry_name) = resolve_sync_entry_name_for_path(sync_dir, old_path) else {
        return Ok(());
    };
    let Some(new_entry_name) = resolve_sync_entry_name_for_path(sync_dir, new_path) else {
        return Ok(());
    };

    if old_entry_name == new_entry_name {
        return Ok(());
    }

    let mut lock_file = load_skill_lock(sync_dir)?;
    let Some(entry) = lock_file.skills.remove(&old_entry_name) else {
        return Ok(());
    };
    lock_file.skills.insert(new_entry_name, entry);
    save_skill_lock(sync_dir, &lock_file)
}

fn remove_skill_lock_entry(sync_dir: &Path, path: &Path) -> Result<(), String> {
    let Some(entry_name) = resolve_sync_entry_name_for_path(sync_dir, path) else {
        return Ok(());
    };

    let mut lock_file = load_skill_lock(sync_dir)?;
    if lock_file.skills.remove(&entry_name).is_some() {
        save_skill_lock(sync_dir, &lock_file)?;
    }

    Ok(())
}

fn load_sync_enabled_skills(
    sync_dir: &Path,
) -> Result<Option<HashMap<String, Vec<String>>>, String> {
    let config_path = get_sync_enabled_skills_path(sync_dir);
    if !config_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
    if let Ok(config) = serde_json::from_str::<SyncEnabledSkillsConfig>(&content) {
        return Ok(Some(config.enabled_skills_by_app));
    }

    if let Ok(map) = serde_json::from_str::<HashMap<String, Vec<String>>>(&content) {
        return Ok(Some(map));
    }

    Err(format!(
        "无法解析同步目录中的启用配置文件: {}",
        config_path.to_string_lossy()
    ))
}

fn save_sync_enabled_skills(
    sync_dir: &Path,
    enabled_skills_by_app: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    fs::create_dir_all(sync_dir).map_err(|e| e.to_string())?;
    let config_path = get_sync_enabled_skills_path(sync_dir);
    let content = serde_json::to_string_pretty(&SyncEnabledSkillsConfig {
        enabled_skills_by_app: enabled_skills_by_app.clone(),
    })
    .map_err(|e| e.to_string())?;
    fs::write(&config_path, content).map_err(|e| e.to_string())
}

fn load_effective_enabled_skills(
    config: &AppConfig,
    sync_dir: &Path,
) -> Result<HashMap<String, Vec<String>>, String> {
    match load_sync_enabled_skills(sync_dir)? {
        Some(value) => Ok(value),
        None => Ok(config.enabled_skills_by_app.clone()),
    }
}

fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(());
    };

    if metadata.file_type().is_symlink() {
        return fs::remove_file(path)
            .or_else(|_| fs::remove_dir(path))
            .map_err(|e| e.to_string());
    }

    if metadata.is_dir() {
        fs::remove_dir_all(path).map_err(|e| e.to_string())
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())
    }
}

fn should_skip_transfer_root_entry(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".DS_Store"
            | ".ckg"
            | ".mcp_gallery_cache"
            | INTERNAL_GIT_REPO_DIR
            | SKILL_LOCK_FILE
    )
}

fn is_local_only_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();

    if lower == ".env" {
        return true;
    }

    if lower.starts_with(".env.")
        && !matches!(
            lower.as_str(),
            ".env.example" | ".env.sample" | ".env.template" | ".env.default" | ".env.defaults"
        )
    {
        return true;
    }

    if matches!(
        lower.as_str(),
        "secrets" | ".secrets" | "private" | ".private"
    ) {
        return true;
    }

    lower.ends_with(".local.json")
        || lower.ends_with(".local.yaml")
        || lower.ends_with(".local.yml")
        || lower.ends_with(".local.toml")
        || lower.ends_with(".local.env")
}

fn is_local_only_relative_path(path: &Path) -> bool {
    path.components().any(|component| match component {
        std::path::Component::Normal(value) => is_local_only_name(&value.to_string_lossy()),
        _ => false,
    })
}

fn relative_copy_path(path: &Path, root: &Path) -> PathBuf {
    if path == root {
        return path
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(PathBuf::new);
    }

    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

fn build_swap_path(path: &Path, label: &str) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("entry");
    let parent = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut attempt = 0usize;
    loop {
        let suffix = if attempt == 0 {
            format!(".skillbox-{}-{}", label, file_name)
        } else {
            format!(".skillbox-{}-{}-{}", label, attempt, file_name)
        };
        let candidate = parent.join(suffix);
        if !candidate.exists() {
            return candidate;
        }
        attempt += 1;
    }
}

fn list_transfer_entries(root: &Path) -> Result<Vec<String>, String> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();

    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if should_skip_transfer_root_entry(&name)
            || name == SYNC_MANIFEST_FILE
            || name == SYNC_ENABLED_SKILLS_FILE
        {
            continue;
        }

        if path.is_file() {
            if is_supported_instruction_file(&path) {
                entries.push(name);
            }
            continue;
        }

        if !collect_skill_entries(&path)?.is_empty() {
            entries.push(name);
        }
    }

    entries.sort_by_key(|value| value.to_lowercase());
    entries.dedup();
    Ok(entries)
}

fn paths_match(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }

    let left_real = left.canonicalize().ok();
    let right_real = right.canonicalize().ok();

    match (left_real, right_real) {
        (Some(left_real), Some(right_real)) => left_real == right_real,
        _ => false,
    }
}

fn resolve_link_target(link_path: &Path) -> Option<PathBuf> {
    let target = fs::read_link(link_path).ok()?;
    if target.is_absolute() {
        Some(target)
    } else {
        link_path.parent().map(|parent| parent.join(target))
    }
}

fn resolve_managed_link_dir(app_id: &str) -> PathBuf {
    get_config_path().join("linked_apps").join(app_id)
}

fn resolve_internal_git_repo_dir(sync_dir: &Path) -> PathBuf {
    sync_dir.join(INTERNAL_GIT_REPO_DIR)
}

fn sanitize_sync_entry_name(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("技能条目不能为空".to_string());
    }

    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(format!("技能条目不能是绝对路径: {}", trimmed));
    }

    if path
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!("技能条目必须是同步目录内的相对路径: {}", trimmed));
    }

    Ok(trimmed.to_string())
}

fn list_sync_dir_entries(sync_dir: &Path) -> Result<Vec<String>, String> {
    if !sync_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for skill in collect_skill_entries(sync_dir)? {
        let skill_path = PathBuf::from(&skill.path);
        let relative = skill_path
            .strip_prefix(sync_dir)
            .map_err(|_| format!("技能路径不在同步目录中: {}", skill.path))?;
        let entry_name = sanitize_sync_entry_name(&relative.to_string_lossy())?;
        if !entries.contains(&entry_name) {
            entries.push(entry_name);
        }
    }

    entries.sort_by_key(|value| value.to_lowercase());
    Ok(entries)
}

fn collect_sync_dir_skill_file_map(sync_dir: &Path) -> Result<HashMap<String, SkillFile>, String> {
    let mut map = HashMap::new();

    for skill in collect_skill_entries(sync_dir)? {
        let skill_path = PathBuf::from(&skill.path);
        let relative = match skill_path.strip_prefix(sync_dir) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let entry_name = sanitize_sync_entry_name(&relative.to_string_lossy())?;
        map.entry(entry_name).or_insert(skill);
    }

    Ok(map)
}

fn collect_sync_dir_skills(sync_dir: &Path) -> Result<Vec<ManagedSkillEntry>, String> {
    let mut skills = Vec::new();

    for skill in collect_skill_entries(sync_dir)? {
        let skill_path = PathBuf::from(&skill.path);
        let relative = skill_path
            .strip_prefix(sync_dir)
            .map_err(|_| format!("技能路径不在同步目录中: {}", skill.path))?;
        let entry_name = sanitize_sync_entry_name(&relative.to_string_lossy())?;

        skills.push(ManagedSkillEntry {
            entry_name,
            name: skill.name,
            path: skill.path,
            size: skill.size,
            modified: skill.modified,
            description: skill.description,
            canonical_name: skill.canonical_name,
            content_hash: skill.content_hash,
            file_count: skill.file_count,
            enabled: false,
        });
    }

    skills.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(skills)
}

fn get_saved_enabled_entries(
    app_id: &str,
    config: &AppConfig,
    sync_dir: &Path,
) -> Result<Vec<String>, String> {
    let available = list_sync_dir_entries(sync_dir)?;
    let available_set: HashSet<String> = available.iter().cloned().collect();
    let enabled_skills_by_app = load_effective_enabled_skills(config, sync_dir)?;

    match enabled_skills_by_app.get(app_id) {
        Some(saved_entries) => {
            let mut entries = Vec::new();
            for value in saved_entries {
                let entry = sanitize_sync_entry_name(value)?;
                if available_set.contains(&entry) && !entries.contains(&entry) {
                    entries.push(entry);
                    continue;
                }

                let nested_prefix = format!("{}/", entry);
                for available in &available {
                    if available.starts_with(&nested_prefix) && !entries.contains(available) {
                        entries.push(available.clone());
                    }
                }
            }
            Ok(entries)
        }
        None => Ok(available),
    }
}

fn save_enabled_entries_for_app(
    config: &mut AppConfig,
    sync_dir: &Path,
    app_id: &str,
    enabled_entries: Vec<String>,
) -> Result<(), String> {
    config
        .enabled_skills_by_app
        .insert(app_id.to_string(), enabled_entries.clone());

    let mut effective_enabled_skills = load_effective_enabled_skills(config, sync_dir)?;
    effective_enabled_skills.insert(app_id.to_string(), enabled_entries);
    save_sync_enabled_skills(sync_dir, &effective_enabled_skills)?;
    save_config(config)
}

fn create_symlink(source: &Path, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, target).map_err(|e| e.to_string())?;
    }

    #[cfg(windows)]
    {
        if source.is_dir() {
            std::os::windows::fs::symlink_dir(source, target).map_err(|e| e.to_string())?;
        } else {
            std::os::windows::fs::symlink_file(source, target).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn yaml_double_quoted(value: &str) -> String {
    let mut escaped = String::new();

    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => {}
            _ => escaped.push(ch),
        }
    }

    format!("\"{}\"", escaped)
}

fn strip_frontmatter(content: &str) -> String {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut lines = content.lines();

    if !matches!(lines.next(), Some(line) if line.trim() == "---") {
        return content.to_string();
    }

    let mut body_lines = Vec::new();
    let mut frontmatter_closed = false;

    for line in lines {
        if !frontmatter_closed {
            if line.trim() == "---" {
                frontmatter_closed = true;
            }
            continue;
        }

        body_lines.push(line);
    }

    if frontmatter_closed {
        body_lines.join("\n")
    } else {
        content.to_string()
    }
}

fn build_kiro_skill_content(content: &str, export_name: &str, fallback_name: &str) -> String {
    let metadata = parse_skill_metadata(content, fallback_name);
    let mut body = strip_frontmatter(content);
    body = body.trim_start_matches('\n').to_string();

    let frontmatter = format!(
        "---\nname: {}\ndescription: {}\n---",
        yaml_double_quoted(export_name),
        yaml_double_quoted(&metadata.description),
    );

    if body.trim().is_empty() {
        frontmatter
    } else {
        format!("{}\n\n{}", frontmatter, body)
    }
}

fn copy_kiro_skill_recursive(
    source: &Path,
    target: &Path,
    export_name: &str,
) -> Result<(), String> {
    if source.is_dir() {
        fs::create_dir_all(target).map_err(|e| e.to_string())?;

        for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let child_source = entry.path();
            let child_target = target.join(entry.file_name());
            copy_kiro_skill_recursive(&child_source, &child_target, export_name)?;
        }

        return Ok(());
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let is_skill_file = source
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("SKILL.md"))
        .unwrap_or(false);

    if is_skill_file {
        let content = fs::read_to_string(source).map_err(|e| e.to_string())?;
        let fallback_name = source
            .parent()
            .and_then(|value| value.file_name())
            .and_then(|value| value.to_str())
            .unwrap_or(export_name);
        let rewritten = build_kiro_skill_content(&content, export_name, fallback_name);
        fs::write(target, rewritten).map_err(|e| e.to_string())?;
        return Ok(());
    }

    fs::copy(source, target).map_err(|e| e.to_string())?;
    Ok(())
}

fn copy_openclaw_skill_recursive(source: &Path, target: &Path) -> Result<(), String> {
    if source.is_dir() {
        fs::create_dir_all(target).map_err(|e| e.to_string())?;

        for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let child_source = entry.path();
            let child_target = target.join(entry.file_name());
            copy_openclaw_skill_recursive(&child_source, &child_target)?;
        }

        return Ok(());
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    fs::copy(source, target).map_err(|e| e.to_string())?;
    Ok(())
}

fn build_kiro_export_name(entry_name: &str, used_names: &mut HashSet<String>) -> String {
    let entry_path = Path::new(entry_name);
    let basename = entry_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(entry_name);

    let mut candidate = normalize_skill_name(basename);

    if candidate.is_empty() {
        candidate = normalize_skill_name(&entry_name.replace(['/', '\\', '.'], "-"));
    }

    if candidate.is_empty() {
        let digest = Sha256::digest(entry_name.as_bytes());
        candidate = format!("skill-{:x}", digest);
        candidate.truncate(14);
    }

    if used_names.insert(candidate.clone()) {
        return candidate;
    }

    let mut expanded = normalize_skill_name(&entry_name.replace(['/', '\\', '.'], "-"));
    if expanded.is_empty() {
        expanded = candidate.clone();
    }

    if used_names.insert(expanded.clone()) {
        return expanded;
    }

    let digest = Sha256::digest(entry_name.as_bytes());
    let suffix = format!("{:x}", digest);
    let unique = format!("{}-{}", candidate, &suffix[..8]);
    used_names.insert(unique.clone());
    unique
}

fn export_kiro_skill_entry(
    source: &Path,
    managed_dir: &Path,
    entry_name: &str,
    used_names: &mut HashSet<String>,
) -> Result<(), String> {
    let export_name = build_kiro_export_name(entry_name, used_names);
    let target_dir = managed_dir.join(&export_name);
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    if source.is_dir() {
        copy_kiro_skill_recursive(source, &target_dir, &export_name)?;
        let skill_file = target_dir.join("SKILL.md");
        if !skill_file.exists() {
            return Err(format!(
                "Kiro 技能目录缺少 SKILL.md: {}",
                source.to_string_lossy()
            ));
        }
        return Ok(());
    }

    if is_supported_instruction_file(source) {
        let content = fs::read_to_string(source).map_err(|e| e.to_string())?;
        let fallback_name = source
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(&export_name);
        let rewritten = build_kiro_skill_content(&content, &export_name, fallback_name);
        fs::write(target_dir.join("SKILL.md"), rewritten).map_err(|e| e.to_string())?;
        return Ok(());
    }

    Err(format!(
        "Kiro 仅支持指令文件或包含 SKILL.md 的目录: {}",
        source.to_string_lossy()
    ))
}

fn export_openclaw_skill_entry(
    source: &Path,
    managed_dir: &Path,
    entry_name: &str,
    used_names: &mut HashSet<String>,
) -> Result<(), String> {
    let export_name = build_kiro_export_name(entry_name, used_names);
    let target_dir = managed_dir.join(&export_name);
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    if source.is_dir() {
        copy_openclaw_skill_recursive(source, &target_dir)?;
        let skill_file = target_dir.join("SKILL.md");
        if !skill_file.exists() {
            return Err(format!(
                "OpenClaw 技能目录缺少 SKILL.md: {}",
                source.to_string_lossy()
            ));
        }
        return Ok(());
    }

    if is_supported_instruction_file(source) {
        let content = fs::read_to_string(source).map_err(|e| e.to_string())?;
        fs::write(target_dir.join("SKILL.md"), content).map_err(|e| e.to_string())?;
        return Ok(());
    }

    Err(format!(
        "OpenClaw 仅支持指令文件或包含 SKILL.md 的目录: {}",
        source.to_string_lossy()
    ))
}

fn populate_managed_skill_dir(
    app_id: &str,
    sync_dir: &Path,
    enabled_entries: &[String],
    managed_dir: &Path,
) -> Result<(), String> {
    fs::create_dir_all(managed_dir).map_err(|e| e.to_string())?;
    let mut exported_used_names = HashSet::new();

    for value in enabled_entries {
        let entry_name = sanitize_sync_entry_name(value)?;
        let source = sync_dir.join(&entry_name);
        if !source.exists() {
            continue;
        }

        match app_id {
            "kiro" | "copilot" => {
                export_kiro_skill_entry(
                    &source,
                    managed_dir,
                    &entry_name,
                    &mut exported_used_names,
                )?;
            }
            "openclaw" => {
                export_openclaw_skill_entry(
                    &source,
                    managed_dir,
                    &entry_name,
                    &mut exported_used_names,
                )?;
            }
            _ => {
                let target = managed_dir.join(&entry_name);
                create_symlink(&source, &target)?;
            }
        }
    }

    Ok(())
}

fn rebuild_managed_skill_dir(
    app_id: &str,
    sync_dir: &Path,
    enabled_entries: &[String],
) -> Result<PathBuf, String> {
    fs::create_dir_all(sync_dir).map_err(|e| e.to_string())?;

    let managed_dir = resolve_managed_link_dir(app_id);
    let next_managed_dir = build_swap_path(&managed_dir, "managed");
    remove_path_if_exists(&next_managed_dir)?;
    fs::create_dir_all(&next_managed_dir).map_err(|e| e.to_string())?;

    if let Err(error) =
        populate_managed_skill_dir(app_id, sync_dir, enabled_entries, &next_managed_dir)
    {
        let _ = remove_path_if_exists(&next_managed_dir);
        return Err(error);
    }

    if managed_dir.exists() {
        if let Err(error) = merge_local_only_paths(&managed_dir, &next_managed_dir, false) {
            let _ = remove_path_if_exists(&next_managed_dir);
            return Err(error);
        }
    }

    let previous_managed_dir = if managed_dir.exists() {
        let backup_dir = build_swap_path(&managed_dir, "managed-backup");
        fs::rename(&managed_dir, &backup_dir).map_err(|e| e.to_string())?;
        Some(backup_dir)
    } else {
        None
    };

    if let Err(error) = fs::rename(&next_managed_dir, &managed_dir) {
        let _ = remove_path_if_exists(&next_managed_dir);
        if let Some(previous_dir) = previous_managed_dir.as_ref() {
            let _ = fs::rename(previous_dir, &managed_dir);
        }
        return Err(error.to_string());
    }

    if let Some(previous_dir) = previous_managed_dir {
        let _ = remove_path_if_exists(&previous_dir);
    }

    Ok(managed_dir)
}

fn ensure_app_points_to_managed_dir(
    skill_dir: &Path,
    backup_dir: &Path,
    managed_dir: &Path,
) -> Result<(), String> {
    if let Ok(metadata) = fs::symlink_metadata(skill_dir) {
        if metadata.file_type().is_symlink() {
            remove_path_if_exists(skill_dir)?;
        } else if !backup_dir.exists() {
            fs::rename(skill_dir, backup_dir).map_err(|e| e.to_string())?;
        }
    }

    if let Some(parent) = skill_dir.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    if !skill_dir.exists() {
        create_symlink(managed_dir, skill_dir)?;
    }

    Ok(())
}

fn cleanup_legacy_skill_paths(app_id: &str) -> Result<(), String> {
    for legacy_path in get_legacy_skill_paths(app_id) {
        let backup_dir = get_backup_path(&legacy_path);

        if let Ok(metadata) = fs::symlink_metadata(&legacy_path) {
            if metadata.file_type().is_symlink() {
                remove_path_if_exists(&legacy_path)?;
            }
        }

        if !legacy_path.exists() && backup_dir.exists() {
            if let Some(parent) = legacy_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::rename(&backup_dir, &legacy_path).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn detect_link_mode(skill_dir: &Path, app_id: &str, config: &AppConfig) -> Option<String> {
    let metadata = fs::symlink_metadata(skill_dir).ok()?;
    if !metadata.file_type().is_symlink() {
        return None;
    }

    let target = resolve_link_target(skill_dir)?;
    let managed_dir = resolve_managed_link_dir(app_id);
    if paths_match(&target, &managed_dir) || target == managed_dir {
        return Some("managed".to_string());
    }

    if let Some(git_path) = config
        .git_path
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        let sync_dir = PathBuf::from(git_path.trim());
        if paths_match(&target, &sync_dir) || target == sync_dir {
            return Some("legacy".to_string());
        }
    }

    Some("legacy".to_string())
}

fn get_enabled_skill_count(app_id: &str, link_mode: Option<&str>, config: &AppConfig) -> usize {
    let Some(git_path) = config
        .git_path
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    else {
        return 0;
    };
    let sync_dir = PathBuf::from(git_path.trim());
    let Ok(entries) = list_sync_dir_entries(&sync_dir) else {
        return 0;
    };

    match link_mode {
        Some("managed") => get_saved_enabled_entries(app_id, config, &sync_dir)
            .map(|value| value.len())
            .unwrap_or(entries.len()),
        Some("legacy") => entries.len(),
        _ => 0,
    }
}

fn get_linked_app_ids(config: &AppConfig) -> Vec<String> {
    let mut app_ids = Vec::new();

    for app in build_known_apps() {
        let path = match resolve_skill_path(&app.id, config) {
            Ok(path) => PathBuf::from(path),
            Err(_) => continue,
        };

        if matches!(
            detect_link_mode(&path, &app.id, config).as_deref(),
            Some("managed")
        ) {
            app_ids.push(app.id.clone());
        }
    }

    for app_id in config.custom_paths.keys() {
        if app_ids.contains(app_id) {
            continue;
        }

        let path = match resolve_skill_path(app_id, config) {
            Ok(path) => PathBuf::from(path),
            Err(_) => continue,
        };

        if matches!(
            detect_link_mode(&path, app_id, config).as_deref(),
            Some("managed")
        ) {
            app_ids.push(app_id.clone());
        }
    }

    app_ids.sort();
    app_ids.dedup();
    app_ids
}

fn rebuild_managed_links_for_all_apps(config: &AppConfig) -> Result<(), String> {
    let Some(git_path) = config
        .git_path
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(());
    };
    let sync_dir = PathBuf::from(git_path.trim());
    if !sync_dir.exists() {
        return Ok(());
    }

    for app_id in get_linked_app_ids(config) {
        let skill_path = resolve_skill_path(&app_id, config)?;
        let skill_dir = PathBuf::from(&skill_path);
        let backup_dir = get_backup_path(&skill_dir);
        let enabled_entries = get_saved_enabled_entries(&app_id, config, &sync_dir)?;
        let managed_dir = rebuild_managed_skill_dir(&app_id, &sync_dir, &enabled_entries)?;
        ensure_app_points_to_managed_dir(&skill_dir, &backup_dir, &managed_dir)?;
    }

    Ok(())
}

fn run_git(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let safe_working_dir = std::env::temp_dir();
    let repo_path_string = repo_path.to_string_lossy().to_string();
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path_string)
        .args(args)
        .current_dir(&safe_working_dir)
        .output()
        .map_err(|e| {
            format!(
                "Failed to run git -C {} {}: {}",
                repo_path.display(),
                args.join(" "),
                e
            )
        })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if stderr.is_empty() { stdout } else { stderr };
        Err(format!(
            "git -C {} {} failed: {}",
            repo_path.display(),
            args.join(" "),
            message
        ))
    }
}

fn push_git_progress_line<F>(
    buffer: &[u8],
    on_progress: &Arc<F>,
    log_lines: &Arc<Mutex<VecDeque<String>>>,
    max_log_lines: usize,
) where
    F: Fn(&str) + Send + Sync + 'static,
{
    let line = String::from_utf8_lossy(buffer).trim().to_string();
    if line.is_empty() {
        return;
    }

    if let Ok(mut lines) = log_lines.lock() {
        if lines.len() == max_log_lines {
            lines.pop_front();
        }
        lines.push_back(line.clone());
    }

    on_progress(&line);
}

fn forward_git_progress_stream<R, F>(
    reader: R,
    on_progress: Arc<F>,
    log_lines: Arc<Mutex<VecDeque<String>>>,
    max_log_lines: usize,
) where
    R: Read,
    F: Fn(&str) + Send + Sync + 'static,
{
    let mut reader = BufReader::new(reader);
    let mut buffer = Vec::new();
    let mut byte = [0_u8; 1];

    loop {
        match reader.read(&mut byte) {
            Ok(0) => {
                if !buffer.is_empty() {
                    push_git_progress_line(&buffer, &on_progress, &log_lines, max_log_lines);
                }
                break;
            }
            Ok(_) => match byte[0] {
                b'\n' | b'\r' => {
                    if !buffer.is_empty() {
                        push_git_progress_line(&buffer, &on_progress, &log_lines, max_log_lines);
                        buffer.clear();
                    }
                }
                value => buffer.push(value),
            },
            Err(_) => break,
        }
    }
}

fn copy_path_recursive(source: &Path, target: &Path) -> Result<(), String> {
    if source.is_dir() {
        fs::create_dir_all(target).map_err(|e| e.to_string())?;

        for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let child_source = entry.path();
            let child_target = target.join(entry.file_name());
            copy_path_recursive(&child_source, &child_target)?;
        }

        Ok(())
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        fs::copy(source, target).map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn copy_path_recursive_without_local_only(
    source: &Path,
    target: &Path,
    root: &Path,
) -> Result<(), String> {
    let relative = relative_copy_path(source, root);
    if !relative.as_os_str().is_empty() && is_local_only_relative_path(&relative) {
        return Ok(());
    }

    if source.is_dir() {
        fs::create_dir_all(target).map_err(|e| e.to_string())?;

        for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let child_source = entry.path();
            let child_target = target.join(entry.file_name());
            copy_path_recursive_without_local_only(&child_source, &child_target, root)?;
        }

        return Ok(());
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    fs::copy(source, target).map_err(|e| e.to_string())?;
    Ok(())
}

fn copy_entry(source: &Path, target: &Path, skip_local_only: bool) -> Result<(), String> {
    if skip_local_only {
        copy_path_recursive_without_local_only(source, target, source)
    } else {
        copy_path_recursive(source, target)
    }
}

fn merge_local_only_paths(
    source_root: &Path,
    target_root: &Path,
    overwrite_existing: bool,
) -> Result<(), String> {
    if !source_root.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(source_root)
        .follow_links(true)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let relative = match entry.path().strip_prefix(source_root) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if relative.as_os_str().is_empty() || !is_local_only_relative_path(relative) {
            continue;
        }

        let target = target_root.join(relative);
        if target.exists() && !overwrite_existing {
            continue;
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn replace_entry_for_sync(
    source: &Path,
    target: &Path,
    skip_source_local_only: bool,
    preserve_target_local_only: bool,
) -> Result<(), String> {
    if !target.exists() {
        return copy_entry(source, target, skip_source_local_only);
    }

    if !preserve_target_local_only {
        remove_path_if_exists(target)?;
        return copy_entry(source, target, skip_source_local_only);
    }

    let preserved_target = build_swap_path(target, "preserve");
    fs::rename(target, &preserved_target).map_err(|e| e.to_string())?;

    if let Err(error) = copy_entry(source, target, skip_source_local_only) {
        let _ = remove_path_if_exists(target);
        let _ = fs::rename(&preserved_target, target);
        return Err(error);
    }

    if let Err(error) = merge_local_only_paths(&preserved_target, target, false) {
        let _ = remove_path_if_exists(target);
        let _ = fs::rename(&preserved_target, target);
        return Err(error);
    }

    remove_path_if_exists(&preserved_target)?;
    Ok(())
}

fn prune_empty_parent_dirs(root: &Path, path: &Path) -> Result<(), String> {
    let mut current = path.parent();

    while let Some(dir) = current {
        if dir == root {
            break;
        }

        let is_empty = fs::read_dir(dir)
            .map_err(|e| e.to_string())?
            .next()
            .transpose()
            .map_err(|e| e.to_string())?
            .is_none();

        if !is_empty {
            break;
        }

        fs::remove_dir(dir).map_err(|e| e.to_string())?;
        current = dir.parent();
    }

    Ok(())
}

fn sync_metadata_file(
    source_root: &Path,
    target_root: &Path,
    file_name: &str,
    remove_if_missing: bool,
) -> Result<(), String> {
    let source = source_root.join(file_name);
    let target = target_root.join(file_name);

    if source.exists() {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::copy(source, target).map_err(|e| e.to_string())?;
        return Ok(());
    }

    if remove_if_missing && target.exists() {
        remove_path_if_exists(&target)?;
    }

    Ok(())
}

fn sync_skill_workspace(
    source_root: &Path,
    target_root: &Path,
    options: SyncWorkspaceOptions,
) -> Result<(), String> {
    fs::create_dir_all(source_root).map_err(|e| e.to_string())?;
    fs::create_dir_all(target_root).map_err(|e| e.to_string())?;

    let source_entries = list_transfer_entries(source_root)?;
    let source_entry_set: HashSet<String> = source_entries.iter().cloned().collect();

    if options.remove_missing_entries {
        for target_entry in list_transfer_entries(target_root)? {
            if source_entry_set.contains(&target_entry) {
                continue;
            }

            let stale_path = target_root.join(&target_entry);
            remove_path_if_exists(&stale_path)?;
            prune_empty_parent_dirs(target_root, &stale_path)?;
        }
    }

    for entry_name in &source_entries {
        let source = source_root.join(entry_name);
        let target = target_root.join(entry_name);
        replace_entry_for_sync(
            &source,
            &target,
            options.skip_source_local_only,
            options.preserve_target_local_only,
        )?;
    }

    save_sync_manifest(target_root, &source_entries)?;
    sync_metadata_file(
        source_root,
        target_root,
        SYNC_ENABLED_SKILLS_FILE,
        options.remove_missing_entries,
    )?;
    sync_metadata_file(
        source_root,
        target_root,
        SKILL_LOCK_FILE,
        options.remove_missing_entries,
    )?;

    Ok(())
}

fn save_sync_manifest(repo_path: &Path, entries: &[String]) -> Result<(), String> {
    let manifest_path = repo_path.join(SYNC_MANIFEST_FILE);
    let content = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    fs::write(manifest_path, content).map_err(|e| e.to_string())
}

fn build_app_skill_lock_entry(app: &SkillApp, skill: &SkillFile) -> SkillLockEntry {
    SkillLockEntry {
        source_type: "app".to_string(),
        source_app_id: Some(app.id.clone()),
        source_app_name: Some(app.name.clone()),
        source_path: Some(skill.path.clone()),
        canonical_name: skill.canonical_name.clone(),
        content_hash: skill.content_hash.clone(),
        file_count: skill.file_count,
    }
}

fn build_unknown_skill_lock_entry(skill: &SkillFile) -> SkillLockEntry {
    SkillLockEntry {
        source_type: "unknown".to_string(),
        source_app_id: None,
        source_app_name: None,
        source_path: Some(skill.path.clone()),
        canonical_name: skill.canonical_name.clone(),
        content_hash: skill.content_hash.clone(),
        file_count: skill.file_count,
    }
}

fn get_skill_base_name(skill: &SkillFile) -> String {
    let skill_path = PathBuf::from(&skill.path);
    skill_path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| {
            let fallback = if skill.canonical_name.is_empty() {
                normalize_skill_name(&skill.name)
            } else {
                skill.canonical_name.clone()
            };

            if skill_path.is_file() {
                format!("{}.md", fallback)
            } else {
                fallback
            }
        })
}

fn make_flat_skill_name(
    skill: &SkillFile,
    app_id: &str,
    used_names: &mut std::collections::HashSet<String>,
) -> String {
    let skill_path = PathBuf::from(&skill.path);
    let original_name = skill_path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| {
            let fallback = if skill.canonical_name.is_empty() {
                normalize_skill_name(&skill.name)
            } else {
                skill.canonical_name.clone()
            };

            if skill_path.is_file() {
                format!("{}.md", fallback)
            } else {
                fallback
            }
        });

    if used_names.insert(original_name.clone()) {
        return original_name;
    }

    let path = Path::new(&original_name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("skill");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    let mut attempt = 1usize;
    loop {
        let candidate = if extension.is_empty() {
            format!("{}--{}-{}", stem, app_id, attempt)
        } else {
            format!("{}--{}-{}.{}", stem, app_id, attempt, extension)
        };

        if used_names.insert(candidate.clone()) {
            return candidate;
        }

        attempt += 1;
    }
}

fn normalized_git_branch(git_config: &GitSyncConfig) -> String {
    let branch = git_config.branch.trim();
    if branch.is_empty() {
        default_git_branch()
    } else {
        branch.to_string()
    }
}

fn with_fresh_internal_git_repo<T, F>(sync_dir: &Path, operation: F) -> Result<T, String>
where
    F: FnOnce(&Path) -> Result<T, String>,
{
    fs::create_dir_all(sync_dir).map_err(|e| e.to_string())?;

    let repo = resolve_internal_git_repo_dir(sync_dir);
    remove_path_if_exists(&repo)?;

    let result = operation(&repo);
    let cleanup_result = remove_path_if_exists(&repo);

    match (result, cleanup_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(cleanup_error)) => Err(format!("清理临时同步仓库失败: {}", cleanup_error)),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(format!(
            "{}\n临时同步仓库清理也失败了: {}",
            error, cleanup_error
        )),
    }
}

fn initialize_temp_push_repo(repo_path: &Path, git_config: &GitSyncConfig) -> Result<(), String> {
    fs::create_dir_all(repo_path).map_err(|e| e.to_string())?;
    run_git(repo_path, &["init"])?;
    run_git(
        repo_path,
        &["remote", "add", "origin", git_config.repo_url.trim()],
    )?;
    Ok(())
}

fn clone_remote_snapshot(
    repo_path: &Path,
    git_config: &GitSyncConfig,
    app_handle: Option<&tauri::AppHandle>,
) -> Result<(), String> {
    let safe_working_dir = std::env::temp_dir();
    let repo_path_string = repo_path.to_string_lossy().to_string();
    let branch = normalized_git_branch(git_config);
    let progress_handle = app_handle.cloned();
    let mut cmd = Command::new("git");
    cmd.args([
        "clone",
        "--progress",
        "--depth",
        "1",
        "--branch",
        &branch,
        "--single-branch",
        git_config.repo_url.trim(),
        &repo_path_string,
    ])
    .current_dir(&safe_working_dir)
    .env("GIT_FLUSH", "1")
    .env("GIT_PROGRESS_DELAY", "1")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "Failed to spawn git clone {}: {}",
            git_config.repo_url.trim(),
            e
        )
    })?;

    let stdout = child.stdout.take().expect("stdout pipe");
    let stderr = child.stderr.take().expect("stderr pipe");
    let on_progress = Arc::new(move |line: &str| {
        emit_git_log(progress_handle.as_ref(), line);
    });
    let stdout_callback = Arc::clone(&on_progress);
    let stderr_callback = Arc::clone(&on_progress);
    let log_lines = Arc::new(Mutex::new(VecDeque::with_capacity(12)));
    let stdout_lines = Arc::clone(&log_lines);
    let stderr_lines = Arc::clone(&log_lines);

    let stdout_thread = thread::spawn(move || {
        forward_git_progress_stream(stdout, stdout_callback, stdout_lines, 12);
    });

    let stderr_thread = thread::spawn(move || {
        forward_git_progress_stream(stderr, stderr_callback, stderr_lines, 12);
    });

    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        let details = log_lines
            .lock()
            .ok()
            .map(|lines| {
                lines
                    .iter()
                    .filter(|line| !line.trim().is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        Err(format!(
            "git clone --progress --depth 1 --branch {} --single-branch {} {} failed with exit code: {:?}{}",
            branch,
            git_config.repo_url.trim(),
            repo_path.display(),
            status.code(),
            if details.is_empty() {
                String::new()
            } else {
                format!("\n{}", details)
            }
        ))
    }
}

fn emit_git_log(app_handle: Option<&tauri::AppHandle>, message: &str) {
    if let Some(handle) = app_handle {
        let _ = handle.emit_all("git-log", message.to_string());
    }
}

fn pull_remote_snapshot_into_sync_dir(
    sync_dir: &Path,
    app_config: &AppConfig,
    app_handle: Option<&tauri::AppHandle>,
    rebuild_links: bool,
) -> Result<String, String> {
    emit_git_log(app_handle, "准备临时同步仓库...");

    let result = with_fresh_internal_git_repo(sync_dir, |repo| {
        emit_git_log(app_handle, "克隆远程仓库...");
        clone_remote_snapshot(repo, &app_config.git_config, app_handle)?;

        emit_git_log(app_handle, "同步工作区...");
        sync_skill_workspace(
            repo,
            sync_dir,
            SyncWorkspaceOptions {
                remove_missing_entries: true,
                skip_source_local_only: true,
                preserve_target_local_only: true,
            },
        )?;

        if rebuild_links {
            emit_git_log(app_handle, "重建应用链接...");
            rebuild_managed_links_for_all_apps(app_config)?;
        }

        Ok(())
    });

    match result {
        Ok(()) => {
            emit_git_log(app_handle, "✓ 完成！");
            Ok("已从远程仓库拉取并更新本地同步目录".to_string())
        }
        Err(error) => Err(error),
    }
}

fn push_sync_dir_snapshot(sync_dir: &Path, app_config: &AppConfig) -> Result<(), String> {
    with_fresh_internal_git_repo(sync_dir, |repo| {
        initialize_temp_push_repo(repo, &app_config.git_config)?;
        sync_skill_workspace(
            sync_dir,
            repo,
            SyncWorkspaceOptions {
                remove_missing_entries: true,
                skip_source_local_only: true,
                preserve_target_local_only: false,
            },
        )?;

        if !commit_repo_changes(repo)? {
            run_git(repo, &["commit", "--allow-empty", "-m", "Sync AI skills"])?;
        }

        let branch = normalized_git_branch(&app_config.git_config);
        let refspec = format!("HEAD:{}", branch);
        run_git(repo, &["push", "--force", "-u", "origin", &refspec])?;
        Ok(())
    })
}

fn commit_repo_changes(repo_path: &Path) -> Result<bool, String> {
    run_git(repo_path, &["add", "."])?;

    let status = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repo_path)
        .status()
        .map_err(|e| format!("Failed to run git diff --cached --quiet: {}", e))?;

    if status.success() {
        return Ok(false);
    }

    // Try normal commit first
    match run_git(repo_path, &["commit", "-m", "Sync AI skills"]) {
        Ok(_) => return Ok(true),
        Err(e)
            if e.contains("cannot lock ref")
                || e.contains("reference already exists")
                || e.contains("unable to resolve HEAD") =>
        {
            // Git repo is corrupted. Re-initialize it.
            let git_dir = repo_path.join(".git");

            // Remove the entire .git directory
            let _ = fs::remove_dir_all(&git_dir);

            // Re-initialize git repo
            run_git(repo_path, &["init"])?;

            // Re-add all files and commit
            run_git(repo_path, &["add", "."])?;
            run_git(repo_path, &["commit", "-m", "Sync AI skills"])?;

            Ok(true)
        }
        Err(e) => Err(e),
    }
}

fn check_link_status(path: &str) -> (bool, Option<String>) {
    let path_obj = PathBuf::from(path);
    if let Ok(metadata) = fs::symlink_metadata(&path_obj) {
        if metadata.file_type().is_symlink() {
            return (true, None);
        }
    }

    let backup_path = get_backup_path(&path_obj);
    if backup_path.exists() {
        return (false, Some(backup_path.to_string_lossy().to_string()));
    }

    (false, None)
}

fn get_backup_path(path: &Path) -> PathBuf {
    let backup_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| {
            if value.eq_ignore_ascii_case("skills") {
                return "skill_backup".to_string();
            }

            match (
                Path::new(value).file_stem().and_then(|item| item.to_str()),
                Path::new(value).extension().and_then(|item| item.to_str()),
            ) {
                (Some(stem), Some(extension)) if !stem.is_empty() && !extension.is_empty() => {
                    format!("{}_backup.{}", stem, extension)
                }
                _ => format!("{}_backup", value),
            }
        })
        .unwrap_or_else(|| "skill_backup".to_string());

    match path.parent() {
        Some(parent) => parent.join(backup_name),
        None => PathBuf::from(backup_name),
    }
}

fn sanitize_skill_name(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("Skill name cannot be empty".to_string());
    }

    if trimmed
        .chars()
        .any(|ch| matches!(ch, '/' | '\\' | ':' | '\0'))
    {
        return Err("Skill name contains unsupported characters".to_string());
    }

    Ok(trimmed.to_string())
}

fn strip_ansi_sequences(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if matches!(chars.peek(), Some('[')) {
                let _ = chars.next();
                while let Some(next) = chars.next() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
                continue;
            }
        }

        result.push(ch);
    }

    result
}

fn command_exists(binary: &str) -> bool {
    Command::new(binary)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn skills_cli_available() -> bool {
    command_exists("skills") || command_exists("npx")
}

fn run_skills_cli(args: &[String], cwd: Option<&Path>) -> Result<String, String> {
    let mut command = if command_exists("skills") {
        let mut command = Command::new("skills");
        command.args(args);
        command
    } else if command_exists("npx") {
        let mut command = Command::new("npx");
        command.arg("--yes").arg("skills").args(args);
        command
    } else {
        return Err("当前系统未检测到 `skills` 或 `npx` 命令，无法使用技能市场。".to_string());
    };

    if let Some(path) = cwd {
        command.current_dir(path);
    }

    let output = command
        .output()
        .map_err(|error| format!("运行 skills 命令失败: {}", error))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let merged = if stderr.trim().is_empty() {
        stdout.to_string()
    } else if stdout.trim().is_empty() {
        stderr.to_string()
    } else {
        format!("{}\n{}", stdout, stderr)
    };
    let cleaned = strip_ansi_sequences(&merged);

    if output.status.success() {
        Ok(cleaned)
    } else {
        Err(cleaned.trim().to_string())
    }
}

fn parse_market_package_id(package_id: &str) -> Result<(String, String, String), String> {
    let (repository, skill_name) = package_id
        .trim()
        .rsplit_once('@')
        .ok_or_else(|| format!("无效的技能包标识: {}", package_id))?;

    if repository.trim().is_empty() || skill_name.trim().is_empty() {
        return Err(format!("无效的技能包标识: {}", package_id));
    }

    let author = repository
        .split('/')
        .next()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(repository)
        .to_string();

    Ok((
        repository.trim().to_string(),
        skill_name.trim().to_string(),
        author,
    ))
}

fn parse_market_search_results(output: &str) -> Vec<MarketSkillRecord> {
    let mut results = Vec::new();
    let mut pending_package: Option<(String, String)> = None;

    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if line.ends_with(" installs") {
            let without_suffix = line.trim_end_matches(" installs").trim();
            if let Some((package_id, downloads)) = without_suffix.rsplit_once(' ') {
                if package_id.contains('@') && package_id.contains('/') {
                    pending_package = Some((
                        package_id.trim().to_string(),
                        format!("{} installs", downloads.trim()),
                    ));
                    continue;
                }
            }
        }

        if let Some((package_id, downloads_label)) = pending_package.take() {
            let url = if line.starts_with('└') {
                line.trim_start_matches('└').trim().to_string()
            } else {
                String::new()
            };

            if let Ok((repository, skill_name, author)) = parse_market_package_id(&package_id) {
                results.push(MarketSkillRecord {
                    package_id,
                    repository: repository.clone(),
                    name: skill_name,
                    author,
                    downloads_label,
                    url,
                    description: String::new(),
                    rating_label: None,
                });
            }
        }
    }

    results
}

fn parse_market_search_results_html(html: &str) -> Vec<MarketSkillRecord> {
    let document = Html::parse_document(html);
    let anchor_selector = Selector::parse("main a[href]").expect("valid search anchor selector");
    let title_selector = Selector::parse("h3").expect("valid search title selector");
    let repository_selector = Selector::parse("p").expect("valid search repository selector");
    let span_selector = Selector::parse("span").expect("valid search span selector");
    let mut seen_package_ids = HashSet::new();
    let mut results = Vec::new();

    for anchor in document.select(&anchor_selector) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        if !href.starts_with('/') {
            continue;
        }

        let segments: Vec<&str> = href.trim_start_matches('/').split('/').collect();
        if segments.len() != 3 || segments.iter().any(|segment| segment.trim().is_empty()) {
            continue;
        }

        let repository = format!("{}/{}", segments[0].trim(), segments[1].trim());
        let skill_name = segments[2].trim().to_string();
        let package_id = format!("{}@{}", repository, skill_name);
        if !seen_package_ids.insert(package_id.clone()) {
            continue;
        }

        let name = anchor
            .select(&title_selector)
            .next()
            .map(|node| normalize_market_text(&node.text().collect::<Vec<_>>().join(" ")))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| skill_name.clone());

        let repository_label = anchor
            .select(&repository_selector)
            .next()
            .map(|node| normalize_market_text(&node.text().collect::<Vec<_>>().join(" ")))
            .filter(|value| value.contains('/'))
            .unwrap_or_else(|| repository.clone());

        let downloads_label = anchor
            .select(&span_selector)
            .filter_map(|node| {
                let value = normalize_market_text(&node.text().collect::<Vec<_>>().join(" "));
                if value.is_empty() {
                    None
                } else {
                    Some(value)
                }
            })
            .last()
            .map(|value| format!("{} installs", value))
            .unwrap_or_default();

        results.push(MarketSkillRecord {
            package_id,
            repository: repository_label.clone(),
            name,
            author: segments[0].trim().to_string(),
            downloads_label,
            url: format!("https://skills.sh{}", href),
            description: String::new(),
            rating_label: None,
        });
    }

    results
}

fn normalize_market_text(content: &str) -> String {
    content.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_recommended_market_skills_html(html: &str) -> Vec<MarketSkillRecord> {
    let document = Html::parse_document(html);
    let anchor_selector = Selector::parse("a[href]").expect("valid anchor selector");
    let name_selector = Selector::parse("h3").expect("valid name selector");
    let span_selector = Selector::parse("span").expect("valid span selector");
    let mut seen_package_ids = HashSet::new();
    let mut results = Vec::new();

    for anchor in document.select(&anchor_selector) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        if !href.starts_with('/') {
            continue;
        }

        let segments: Vec<&str> = href.trim_start_matches('/').split('/').collect();
        if segments.len() != 3 || segments.iter().any(|segment| segment.trim().is_empty()) {
            continue;
        }

        let repository = format!("{}/{}", segments[0].trim(), segments[1].trim());
        let skill_name = segments[2].trim().to_string();
        let package_id = format!("{}@{}", repository, skill_name);
        if !seen_package_ids.insert(package_id.clone()) {
            continue;
        }

        let name = anchor
            .select(&name_selector)
            .next()
            .map(|node| normalize_market_text(&node.text().collect::<Vec<_>>().join(" ")))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| skill_name.clone());

        let downloads_value = anchor
            .select(&span_selector)
            .filter_map(|node| {
                let value = normalize_market_text(&node.text().collect::<Vec<_>>().join(" "));
                if value.is_empty() {
                    None
                } else {
                    Some(value)
                }
            })
            .last();

        let Some(downloads_value) = downloads_value else {
            continue;
        };

        results.push(MarketSkillRecord {
            package_id,
            repository,
            name,
            author: segments[0].trim().to_string(),
            downloads_label: format!("{} installs", downloads_value),
            url: format!("https://skills.sh{}", href),
            description: String::new(),
            rating_label: None,
        });

        if results.len() >= MAX_RECOMMENDED_MARKET_SKILLS {
            break;
        }
    }

    results
}

fn create_market_temp_dir(skill_name: &str, purpose: &str) -> Result<PathBuf, String> {
    let temp_dir = std::env::temp_dir().join(format!(
        "skillbox-market-{}-{}-{}",
        purpose,
        skill_name,
        chrono::Utc::now().timestamp_millis()
    ));
    fs::create_dir_all(&temp_dir).map_err(|error| error.to_string())?;
    Ok(temp_dir)
}

fn build_market_blocking_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(format!("SkillBox/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("创建市场客户端失败: {}", error))
}

fn parse_github_repository(repository: &str) -> Result<(&str, &str), String> {
    repository
        .split_once('/')
        .ok_or_else(|| format!("无效的 GitHub 仓库标识: {}", repository))
}

fn get_github_repository_info(
    client: &reqwest::blocking::Client,
    repository: &str,
) -> Result<GitHubRepositoryInfo, String> {
    let (owner, repo) = parse_github_repository(repository)?;
    let mut url = reqwest::Url::parse("https://api.github.com/repos")
        .map_err(|error| format!("创建 GitHub API 地址失败: {}", error))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "创建 GitHub API 地址失败".to_string())?;
        segments.push(owner);
        segments.push(repo);
    }

    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .map_err(|error| format!("请求 GitHub 仓库信息失败: {}", error))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub 仓库信息返回异常状态: {}",
            response.status()
        ));
    }

    response
        .json::<GitHubRepositoryInfo>()
        .map_err(|error| format!("解析 GitHub 仓库信息失败: {}", error))
}

fn download_github_repository_archive(
    client: &reqwest::blocking::Client,
    repository: &str,
    branch: &str,
) -> Result<Vec<u8>, String> {
    let (owner, repo) = parse_github_repository(repository)?;
    let mut url = reqwest::Url::parse("https://api.github.com/repos")
        .map_err(|error| format!("创建 GitHub 下载地址失败: {}", error))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "创建 GitHub 下载地址失败".to_string())?;
        segments.push(owner);
        segments.push(repo);
        segments.push("zipball");
        segments.push(branch);
    }

    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .map_err(|error| format!("下载 GitHub 技能归档失败: {}", error))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub 技能归档返回异常状态: {}",
            response.status()
        ));
    }

    response
        .bytes()
        .map(|bytes| bytes.to_vec())
        .map_err(|error| format!("读取 GitHub 技能归档失败: {}", error))
}

fn extract_market_archive(archive_bytes: &[u8], output_dir: &Path) -> Result<(), String> {
    let reader = Cursor::new(archive_bytes);
    let mut archive =
        ZipArchive::new(reader).map_err(|error| format!("解析技能归档失败: {}", error))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("读取技能归档条目失败: {}", error))?;
        let Some(enclosed_path) = entry.enclosed_name().map(|path| path.to_path_buf()) else {
            continue;
        };

        let relative_path = enclosed_path.iter().skip(1).collect::<PathBuf>();
        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let target_path = output_dir.join(relative_path);
        if entry.is_dir() {
            fs::create_dir_all(&target_path)
                .map_err(|error| format!("创建技能目录失败: {}", error))?;
            continue;
        }

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("创建技能目录失败: {}", error))?;
        }

        let mut output_file = fs::File::create(&target_path)
            .map_err(|error| format!("写入技能文件失败: {}", error))?;
        std::io::copy(&mut entry, &mut output_file)
            .map_err(|error| format!("写入技能文件失败: {}", error))?;

        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;

            let _ = fs::set_permissions(&target_path, fs::Permissions::from_mode(mode));
        }
    }

    Ok(())
}

fn find_installed_market_skill_dir(temp_dir: &Path, skill_name: &str) -> Result<PathBuf, String> {
    [
        temp_dir.join("skills").join(skill_name),
        temp_dir.join(".agents").join("skills").join(skill_name),
    ]
    .into_iter()
    .find(|path| path.exists())
    .ok_or_else(|| format!("未找到安装后的技能目录：{}", skill_name))
}

fn install_market_skill_into_temp_via_cli(
    repository: &str,
    skill_name: &str,
    temp_dir: &Path,
) -> Result<PathBuf, String> {
    fs::write(
        temp_dir.join("package.json"),
        "{\n  \"name\": \"skillbox-market-install\",\n  \"private\": true\n}\n",
    )
    .map_err(|error| format!("创建临时安装目录失败: {}", error))?;

    run_skills_cli(
        &vec![
            "add".to_string(),
            repository.to_string(),
            "--skill".to_string(),
            skill_name.to_string(),
            "--copy".to_string(),
            "-y".to_string(),
        ],
        Some(temp_dir),
    )?;

    find_installed_market_skill_dir(temp_dir, skill_name)
}

fn install_market_skill_into_temp_native(
    repository: &str,
    skill_name: &str,
    temp_dir: &Path,
) -> Result<PathBuf, String> {
    let client = build_market_blocking_client()?;
    let repository_info = get_github_repository_info(&client, repository)?;
    let archive_bytes =
        download_github_repository_archive(&client, repository, &repository_info.default_branch)?;
    extract_market_archive(&archive_bytes, temp_dir)?;
    find_installed_market_skill_dir(temp_dir, skill_name)
}

fn install_market_skill_into_temp(
    repository: &str,
    skill_name: &str,
    temp_dir: &Path,
) -> Result<PathBuf, String> {
    if skills_cli_available() {
        match install_market_skill_into_temp_via_cli(repository, skill_name, temp_dir) {
            Ok(path) => return Ok(path),
            Err(cli_error) => {
                return install_market_skill_into_temp_native(repository, skill_name, temp_dir)
                    .map_err(|native_error| {
                        format!(
                            "通过 skills CLI 安装失败：{}；内置安装兜底也失败：{}",
                            cli_error, native_error
                        )
                    });
            }
        }
    }

    install_market_skill_into_temp_native(repository, skill_name, temp_dir)
}

fn search_skill_market_native(query: &str) -> Result<Vec<MarketSkillRecord>, String> {
    let client = build_market_blocking_client()?;
    let response = client
        .get("https://skills.sh/")
        .query(&[("q", query)])
        .send()
        .map_err(|error| format!("请求 skills.sh 搜索失败: {}", error))?;

    if !response.status().is_success() {
        return Err(format!("skills.sh 搜索返回异常状态: {}", response.status()));
    }

    let html = response
        .text()
        .map_err(|error| format!("读取 skills.sh 搜索响应失败: {}", error))?;
    Ok(parse_market_search_results_html(&html))
}

fn strip_markdown_frontmatter(content: &str) -> String {
    let trimmed = content.trim_start_matches('\u{feff}').trim();
    let mut lines = trimmed.lines();
    if !matches!(lines.next().map(|line| line.trim()), Some("---")) {
        return trimmed.to_string();
    }

    let all_lines: Vec<&str> = trimmed.lines().collect();
    for index in 1..all_lines.len() {
        if all_lines[index].trim() == "---" {
            return all_lines[index + 1..].join("\n").trim().to_string();
        }
    }

    trimmed.to_string()
}

fn load_market_skill_readme(skill_dir: &Path) -> Result<String, String> {
    let candidates = [
        skill_dir.join("SKILL.md"),
        skill_dir.join("README.md"),
        skill_dir.join("skill.md"),
    ];

    for candidate in candidates {
        if candidate.exists() {
            let content = fs::read_to_string(&candidate)
                .map_err(|error| format!("读取技能说明失败: {}", error))?;
            let normalized = strip_markdown_frontmatter(&content);
            if !normalized.trim().is_empty() {
                return Ok(normalized);
            }
        }
    }

    let markdown_file = WalkDir::new(skill_dir)
        .into_iter()
        .filter_map(Result::ok)
        .find(|entry| {
            entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(|value| value.eq_ignore_ascii_case("md"))
                    .unwrap_or(false)
        })
        .map(|entry| entry.into_path());

    if let Some(path) = markdown_file {
        let content =
            fs::read_to_string(&path).map_err(|error| format!("读取技能说明失败: {}", error))?;
        let normalized = strip_markdown_frontmatter(&content);
        if !normalized.trim().is_empty() {
            return Ok(normalized);
        }
    }

    Err("当前技能没有可显示的 README / SKILL.md".to_string())
}

fn refresh_sync_dir_metadata(sync_dir: &Path, config: &AppConfig) -> Result<(), String> {
    let mut entries = list_sync_dir_entries(sync_dir)?;
    entries.sort();
    entries.dedup();
    save_sync_manifest(sync_dir, &entries)?;

    let mut next_lock = load_skill_lock(sync_dir)?;
    let skill_files = collect_sync_dir_skill_file_map(sync_dir)?;
    next_lock.skills.clear();
    for (entry_name, skill) in &skill_files {
        next_lock
            .skills
            .insert(entry_name.clone(), build_unknown_skill_lock_entry(skill));
    }
    save_skill_lock(sync_dir, &next_lock)?;

    let effective_enabled_skills = load_effective_enabled_skills(config, sync_dir)?;
    save_sync_enabled_skills(sync_dir, &effective_enabled_skills)?;
    rebuild_managed_links_for_all_apps(config)?;

    Ok(())
}

#[tauri::command]
async fn search_skill_market(query: String) -> Result<Vec<MarketSkillRecord>, String> {
    tokio::task::spawn_blocking(move || {
        let trimmed_query = query.trim().to_string();
        if trimmed_query.is_empty() {
            return Ok(Vec::new());
        }

        if skills_cli_available() {
            match run_skills_cli(&vec!["find".to_string(), trimmed_query.clone()], None) {
                Ok(output) => {
                    if output.contains(&format!("No skills found for \"{}\"", trimmed_query)) {
                        return Ok(Vec::new());
                    }

                    return Ok(parse_market_search_results(&output));
                }
                Err(cli_error) => {
                    return search_skill_market_native(&trimmed_query).map_err(|native_error| {
                        format!(
                            "通过 skills CLI 搜索失败：{}；内置搜索兜底也失败：{}",
                            cli_error, native_error
                        )
                    });
                }
            }
        }

        search_skill_market_native(&trimmed_query)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn get_recommended_market_skills() -> Result<Vec<MarketSkillRecord>, String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("SkillBox/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("创建 skills.sh 客户端失败: {}", error))?;

    let response = client
        .get(MARKET_TRENDING_URL)
        .header(
            reqwest::header::ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .send()
        .await
        .map_err(|error| format!("请求 skills.sh Trending 失败: {}", error))?;

    if !response.status().is_success() {
        return Err(format!(
            "skills.sh Trending 返回异常状态: {}",
            response.status()
        ));
    }

    let html = response
        .text()
        .await
        .map_err(|error| format!("读取 skills.sh Trending 响应失败: {}", error))?;
    let skills = parse_recommended_market_skills_html(&html);
    if skills.is_empty() {
        return Err("未能从 skills.sh Trending (24h) 页面解析到技能列表".to_string());
    }

    Ok(skills)
}

#[tauri::command]
async fn get_market_skill_detail(package_id: String) -> Result<MarketSkillDetail, String> {
    tokio::task::spawn_blocking(move || {
        let (repository, skill_name, _) = parse_market_package_id(&package_id)?;
        let temp_dir = create_market_temp_dir(&skill_name, "detail")?;

        let detail_result = (|| -> Result<MarketSkillDetail, String> {
            let skill_dir = install_market_skill_into_temp(&repository, &skill_name, &temp_dir)?;
            let readme = load_market_skill_readme(&skill_dir)?;

            Ok(MarketSkillDetail {
                package_id: package_id.clone(),
                readme,
            })
        })();

        let _ = fs::remove_dir_all(&temp_dir);
        detail_result
    })
    .await
    .map_err(|error| error.to_string())?
}

fn install_skill_market_internal(repo_path: String, package_id: String) -> Result<String, String> {
    let trimmed_repo_path = repo_path.trim();
    if trimmed_repo_path.is_empty() {
        return Err("请先选择本地同步目录".to_string());
    }

    let sync_dir = PathBuf::from(trimmed_repo_path);
    probe_directory_access(&sync_dir)?;
    fs::create_dir_all(&sync_dir).map_err(|error| error.to_string())?;

    let (repository, skill_name, _) = parse_market_package_id(&package_id)?;
    let target_path = sync_dir.join(&skill_name);
    if target_path.exists() {
        return Err(format!("本地同步目录中已存在技能：{}", skill_name));
    }

    let temp_dir = create_market_temp_dir(&skill_name, "install")?;

    let cleanup_result = (|| -> Result<String, String> {
        let source_path = install_market_skill_into_temp(&repository, &skill_name, &temp_dir)?;

        copy_path_recursive(&source_path, &target_path)?;

        let config = load_config();
        refresh_sync_dir_metadata(&sync_dir, &config)?;

        Ok(skill_name.clone())
    })();

    let _ = fs::remove_dir_all(&temp_dir);
    cleanup_result.map(|installed_name| format!("已安装 {} 到本地同步目录", installed_name))
}

#[tauri::command]
async fn install_skill_market(repo_path: String, package_id: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || install_skill_market_internal(repo_path, package_id))
        .await
        .map_err(|error| error.to_string())?
}

fn is_launchable_target(path: &Path) -> bool {
    path.exists()
        && (path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("app"))
            .unwrap_or(false)
            || path.is_file())
}

#[cfg(target_os = "macos")]
fn macos_bundle_candidates(app: &KnownApp) -> Vec<PathBuf> {
    let mut bundle_names = Vec::new();

    for marker in &app.install_markers {
        if marker
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("app"))
            .unwrap_or(false)
        {
            if let Some(file_name) = marker.file_name() {
                let candidate = file_name.to_string_lossy().to_string();
                if !bundle_names.contains(&candidate) {
                    bundle_names.push(candidate);
                }
            }
        }
    }

    let base_name = app.name.trim();
    let default_bundle = format!("{}.app", base_name);
    if !bundle_names.contains(&default_bundle) {
        bundle_names.push(default_bundle);
    }

    let cn_variants = if let Some(stripped) = base_name.strip_suffix(" CN") {
        vec![
            format!("{}.app", stripped.trim()),
            format!("{} CN.app", stripped.trim()),
        ]
    } else {
        vec![format!("{} CN.app", base_name)]
    };

    for candidate in cn_variants {
        if !bundle_names.contains(&candidate) {
            bundle_names.push(candidate);
        }
    }

    let mut roots = vec![PathBuf::from("/Applications")];
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Applications"));
    }

    let mut candidates = Vec::new();
    for root in roots {
        for bundle_name in &bundle_names {
            candidates.push(root.join(bundle_name));
        }
    }

    candidates
}

fn resolve_launch_target(app: &KnownApp) -> Option<PathBuf> {
    if let Some(target) = app
        .install_markers
        .iter()
        .find(|path| is_launchable_target(path))
        .cloned()
    {
        return Some(target);
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(target) = macos_bundle_candidates(app)
            .into_iter()
            .find(|path| is_launchable_target(path))
        {
            return Some(target);
        }
    }

    None
}

fn app_has_install_marker(app: &KnownApp) -> bool {
    app.install_markers.iter().any(|value| value.exists()) || {
        #[cfg(target_os = "macos")]
        {
            macos_bundle_candidates(app)
                .into_iter()
                .any(|path| path.exists())
        }

        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
}

fn open_system_target(target: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(target);
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", ""]);
        command.arg(target);
        command
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(target);
        command
    };

    let status = command
        .status()
        .map_err(|error| format!("Failed to open {}: {}", target.display(), error))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("Failed to open {}", target.display()))
    }
}

#[tauri::command]
fn open_path_in_file_manager(path: String) -> Result<(), String> {
    let target = PathBuf::from(&path);
    if !target.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    #[cfg(target_os = "macos")]
    {
        let metadata = fs::symlink_metadata(&target)
            .map_err(|error| format!("Failed to inspect {}: {}", target.display(), error))?;

        if metadata.file_type().is_symlink() {
            let status = Command::new("open")
                .args(["-R"])
                .arg(&target)
                .status()
                .map_err(|error| format!("Failed to reveal {}: {}", target.display(), error))?;

            if status.success() {
                return Ok(());
            }

            return Err(format!("Failed to reveal {}", target.display()));
        }
    }

    open_system_target(&target)
}

#[tauri::command]
fn launch_app(app_id: String) -> Result<(), String> {
    let app = find_known_app(&app_id).ok_or_else(|| format!("App {} not found", app_id))?;

    let target = resolve_launch_target(&app)
        .ok_or_else(|| format!("No launchable application bundle found for {}", app.name))?;

    open_system_target(&target)
}

#[tauri::command]
fn scan_apps() -> Result<(Vec<SkillApp>, String), String> {
    let config = load_config();
    let git_path = config.git_path.clone().unwrap_or_default();
    let mut apps = Vec::new();

    for app in build_known_apps() {
        let custom_path = config.custom_paths.get(&app.id).cloned();
        let path = resolve_skill_path(&app.id, &config)?;
        let (is_linked, backup_path) = check_link_status(&path);
        let link_mode = detect_link_mode(Path::new(&path), &app.id, &config);
        let is_installed = custom_path
            .as_ref()
            .map(|value| PathBuf::from(value).exists())
            .unwrap_or(false)
            || app.skill_paths.iter().any(|value| value.exists())
            || app_has_install_marker(&app)
            || backup_path.is_some();
        let enabled_skill_count = get_enabled_skill_count(&app.id, link_mode.as_deref(), &config);

        apps.push(SkillApp {
            id: app.id.clone(),
            name: app.name,
            path,
            icon: app.icon,
            skill_count: 0,
            enabled_skill_count,
            is_linked,
            is_installed,
            is_custom: false,
            backup_path,
            custom_path,
            link_mode,
        });
    }

    for (id, custom_path) in &config.custom_paths {
        if !apps.iter().any(|a| a.id == *id) {
            let is_installed = PathBuf::from(custom_path).exists();
            let (is_linked, backup_path) = check_link_status(custom_path);
            let link_mode = detect_link_mode(Path::new(custom_path), id, &config);
            let enabled_skill_count = get_enabled_skill_count(id, link_mode.as_deref(), &config);

            apps.push(SkillApp {
                id: id.clone(),
                name: capitalize_first(id),
                path: custom_path.clone(),
                icon: "📁".to_string(),
                skill_count: 0,
                enabled_skill_count,
                is_linked,
                is_installed,
                is_custom: true,
                backup_path,
                custom_path: Some(custom_path.clone()),
                link_mode,
            });
        }
    }

    Ok((apps, git_path))
}

fn capitalize_first(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().to_string() + &chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[tauri::command]
fn scan_skills(app_id: String) -> Result<Vec<SkillFile>, String> {
    let config = load_config();
    let path = resolve_skill_path(&app_id, &config)?;
    collect_skill_entries(Path::new(&path))
}

#[tauri::command]
fn get_app_enabled_skills(
    app_id: String,
    git_path: String,
) -> Result<AppEnabledSkillsResponse, String> {
    let mut config = load_config();
    let sync_dir = PathBuf::from(git_path.trim());
    if git_path.trim().is_empty() {
        return Err("请先配置本地同步目录".to_string());
    }

    let mut skills = collect_sync_dir_skills(&sync_dir)?;
    let skill_path = resolve_skill_path(&app_id, &config)?;
    let link_mode = detect_link_mode(Path::new(&skill_path), &app_id, &config)
        .unwrap_or_else(|| "managed".to_string());
    let enabled_entries = get_saved_enabled_entries(&app_id, &config, &sync_dir)?;
    let enabled_set: HashSet<String> = enabled_entries.iter().cloned().collect();

    for skill in &mut skills {
        skill.enabled = enabled_set.contains(&skill.entry_name);
    }

    let effective_enabled_skills = load_effective_enabled_skills(&config, &sync_dir)?;
    if !effective_enabled_skills.contains_key(&app_id) {
        save_enabled_entries_for_app(&mut config, &sync_dir, &app_id, enabled_entries.clone())?;
    }

    Ok(AppEnabledSkillsResponse {
        app_id,
        link_mode,
        enabled_entries,
        skills,
    })
}

#[tauri::command]
fn save_app_enabled_skills(
    app_id: String,
    git_path: String,
    enabled_entries: Vec<String>,
) -> Result<(), String> {
    if git_path.trim().is_empty() {
        return Err("请先配置本地同步目录".to_string());
    }

    let sync_dir = PathBuf::from(git_path.trim());
    let mut config = load_config();
    let skill_path = resolve_skill_path(&app_id, &config)?;
    let skill_dir = PathBuf::from(&skill_path);
    let backup_dir = get_backup_path(&skill_dir);
    let available_entries = list_sync_dir_entries(&sync_dir)?;
    let available_set: HashSet<String> = available_entries.iter().cloned().collect();

    let mut sanitized_entries = Vec::new();
    for value in enabled_entries {
        let entry_name = sanitize_sync_entry_name(&value)?;
        if available_set.contains(&entry_name) && !sanitized_entries.contains(&entry_name) {
            sanitized_entries.push(entry_name);
        }
    }

    let managed_dir = rebuild_managed_skill_dir(&app_id, &sync_dir, &sanitized_entries)?;
    ensure_app_points_to_managed_dir(&skill_dir, &backup_dir, &managed_dir)?;
    save_enabled_entries_for_app(&mut config, &sync_dir, &app_id, sanitized_entries)
}

#[tauri::command]
fn rename_skill(skill_path: String, new_name: String) -> Result<String, String> {
    let source = PathBuf::from(&skill_path);
    if !source.exists() {
        return Err(format!("Skill path does not exist: {}", skill_path));
    }

    let safe_name = sanitize_skill_name(&new_name)?;
    let parent = source
        .parent()
        .ok_or_else(|| "Unable to resolve parent directory".to_string())?;

    let target = if source.is_dir() {
        parent.join(&safe_name)
    } else {
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();

        if extension.is_empty() {
            parent.join(&safe_name)
        } else {
            parent.join(format!("{}.{}", safe_name, extension))
        }
    };

    if target == source {
        return Ok(skill_path);
    }

    if target.exists() {
        return Err(format!(
            "Target already exists: {}",
            target.to_string_lossy()
        ));
    }

    fs::rename(&source, &target).map_err(|e| e.to_string())?;
    let config = load_config();
    if let Some(git_path) = config
        .git_path
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        let sync_dir = PathBuf::from(git_path.trim());
        if target.starts_with(&sync_dir) || source.starts_with(&sync_dir) {
            rename_skill_lock_entry(&sync_dir, &source, &target)?;
            rebuild_managed_links_for_all_apps(&config)?;
        }
    }
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
fn delete_skill(skill_path: String) -> Result<(), String> {
    let target = PathBuf::from(&skill_path);
    if !target.exists() {
        return Err(format!("Skill path does not exist: {}", skill_path));
    }

    let metadata = fs::symlink_metadata(&target).map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink() {
        let result = fs::remove_file(&target)
            .or_else(|_| fs::remove_dir(&target))
            .map_err(|e| e.to_string());
        if result.is_ok() {
            let config = load_config();
            if let Some(git_path) = config
                .git_path
                .as_ref()
                .filter(|value| !value.trim().is_empty())
            {
                let sync_dir = PathBuf::from(git_path.trim());
                if target.starts_with(&sync_dir) {
                    remove_skill_lock_entry(&sync_dir, &target)?;
                    rebuild_managed_links_for_all_apps(&config)?;
                }
            }
        }
        return result;
    }

    let result = if metadata.is_dir() {
        fs::remove_dir_all(&target).map_err(|e| e.to_string())
    } else {
        fs::remove_file(&target).map_err(|e| e.to_string())
    };

    if result.is_ok() {
        let config = load_config();
        if let Some(git_path) = config
            .git_path
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            let sync_dir = PathBuf::from(git_path.trim());
            if target.starts_with(&sync_dir) {
                remove_skill_lock_entry(&sync_dir, &target)?;
                rebuild_managed_links_for_all_apps(&config)?;
            }
        }
    }

    result
}

#[tauri::command]
fn set_custom_path(app_id: String, custom_path: Option<String>) -> Result<(), String> {
    let mut config = load_config();

    if let Some(path) = custom_path {
        if !PathBuf::from(&path).exists() {
            return Err(format!("Path does not exist: {}", path));
        }
        config.custom_paths.insert(app_id, path);
    } else {
        config.custom_paths.remove(&app_id);
    }

    save_config(&config)
}

#[tauri::command]
fn add_custom_app(name: String, path: String) -> Result<(), String> {
    let id = name.to_lowercase().replace(" ", "_");

    if !PathBuf::from(&path).exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    let mut config = load_config();
    config.custom_paths.insert(id, path);
    save_config(&config)
}

fn sync_to_git_internal(repo_path: &str) -> Result<(), String> {
    let repo = PathBuf::from(repo_path);
    if !repo.exists() {
        fs::create_dir_all(&repo).map_err(|e| e.to_string())?;
    }

    let config = load_config();
    let apps = scan_apps().map_err(|e| e)?;
    let repo_real = repo.canonicalize().unwrap_or(repo.clone());
    let existing_lock = load_skill_lock(&repo)?;

    let mut written_entries = Vec::new();
    let mut used_names = std::collections::HashSet::new();
    let mut seen_skill_dirs = std::collections::HashSet::new();
    let mut next_lock = SkillLockFile {
        version: existing_lock.version.max(1),
        skills: BTreeMap::new(),
    };

    // 收集同步目录中已有的技能名称
    if let Ok(existing_entries) = fs::read_dir(&repo) {
        for entry in existing_entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            // 排除隐藏目录和特殊文件
            if !name.starts_with('.')
                && name != SYNC_MANIFEST_FILE
                && name != SYNC_ENABLED_SKILLS_FILE
                && name != SKILL_LOCK_FILE
            {
                used_names.insert(name.clone());
                if let Some(lock_entry) = existing_lock.skills.get(&name) {
                    next_lock.skills.insert(name, lock_entry.clone());
                }
            }
        }
    }

    for app in apps.0 {
        if app.is_linked {
            continue;
        }

        if app.is_installed {
            let skill_dir = PathBuf::from(&app.path);
            if skill_dir.exists() {
                let skill_dir_real = skill_dir.canonicalize().unwrap_or(skill_dir.clone());
                // 跳过软链接应用（技能已在同步目录中）
                if skill_dir_real == repo_real || skill_dir_real.starts_with(&repo_real) {
                    continue;
                }

                let skill_dir_key = skill_dir_real.to_string_lossy().to_string();
                if !seen_skill_dirs.insert(skill_dir_key) {
                    continue;
                }

                let entries = collect_skill_entries(&skill_dir)?;
                for skill in entries {
                    let source = PathBuf::from(&skill.path);
                    if !source.exists() {
                        continue;
                    }

                    let source_real = source.canonicalize().unwrap_or(source.clone());
                    if source_real == repo_real || source_real.starts_with(&repo_real) {
                        continue;
                    }

                    // 获取基础名称
                    let base_name = get_skill_base_name(&skill);

                    // 如果同步目录里已经有同名文件/目录，说明是上次同步的结果，
                    // 直接复用该名称，跳过复制，避免重复生成 技能名-应用名 副本。
                    if used_names.contains(&base_name) {
                        let existing_target = repo.join(&base_name);
                        if existing_target.exists() {
                            written_entries.push(base_name.clone());
                            next_lock
                                .skills
                                .entry(base_name)
                                .or_insert_with(|| build_app_skill_lock_entry(&app, &skill));
                            continue;
                        }
                    }

                    let flat_name = make_flat_skill_name(&skill, &app.id, &mut used_names);

                    let target = repo.join(&flat_name);
                    let target_real = target.canonicalize().unwrap_or(target.clone());
                    if source_real == target_real {
                        continue;
                    }

                    copy_entry(&source, &target, true)?;
                    written_entries.push(flat_name.clone());
                    next_lock
                        .skills
                        .insert(flat_name, build_app_skill_lock_entry(&app, &skill));
                }
            }
        }
    }

    // 将同步目录中已有的技能也加入 manifest
    for name in used_names {
        written_entries.push(name);
    }

    written_entries.sort();
    written_entries.dedup();
    let final_entry_set: HashSet<String> = written_entries.iter().cloned().collect();
    let final_skill_files = collect_sync_dir_skill_file_map(&repo)?;
    for entry_name in &written_entries {
        if next_lock.skills.contains_key(entry_name) {
            continue;
        }
        if let Some(skill) = final_skill_files.get(entry_name) {
            next_lock
                .skills
                .insert(entry_name.clone(), build_unknown_skill_lock_entry(skill));
        }
    }
    next_lock
        .skills
        .retain(|entry_name, _| final_entry_set.contains(entry_name));

    save_sync_manifest(&repo, &written_entries)?;
    let effective_enabled_skills = load_effective_enabled_skills(&config, &repo)?;
    save_sync_enabled_skills(&repo, &effective_enabled_skills)?;
    save_skill_lock(&repo, &next_lock)?;
    rebuild_managed_links_for_all_apps(&config)?;

    Ok(())
}

#[tauri::command]
async fn sync_to_git(repo_path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || sync_to_git_internal(&repo_path))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn get_git_config() -> Result<GitSyncConfig, String> {
    let config = load_config();
    Ok(config.git_config)
}

#[tauri::command]
fn save_git_config(config: GitSyncConfig) -> Result<(), String> {
    let mut app_config = load_config();
    app_config.git_config = GitSyncConfig {
        repo_url: config.repo_url.trim().to_string(),
        branch: if config.branch.trim().is_empty() {
            default_git_branch()
        } else {
            config.branch.trim().to_string()
        },
    };
    save_config(&app_config)
}

#[tauri::command]
async fn git_push(repo_path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let app_config = load_config();
        if app_config.git_config.repo_url.trim().is_empty() {
            return Err("请先保存仓库地址".to_string());
        }

        let sync_dir = PathBuf::from(&repo_path);
        fs::create_dir_all(&sync_dir).map_err(|e| e.to_string())?;
        push_sync_dir_snapshot(&sync_dir, &app_config)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn git_pull(repo_path: String, app_handle: tauri::AppHandle) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let app_config = load_config();
        if app_config.git_config.repo_url.trim().is_empty() {
            return Err("请先保存仓库地址".to_string());
        }

        let sync_dir = PathBuf::from(&repo_path);
        fs::create_dir_all(&sync_dir).map_err(|e| e.to_string())?;
        pull_remote_snapshot_into_sync_dir(&sync_dir, &app_config, Some(&app_handle), true)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn git_sync(repo_path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let app_config = load_config();
        if app_config.git_config.repo_url.trim().is_empty() {
            return Err("请先保存仓库地址".to_string());
        }

        let sync_dir = PathBuf::from(&repo_path);
        fs::create_dir_all(&sync_dir).map_err(|e| e.to_string())?;
        pull_remote_snapshot_into_sync_dir(&sync_dir, &app_config, None, false)?;
        sync_to_git_internal(&repo_path)?;
        push_sync_dir_snapshot(&sync_dir, &app_config)?;
        Ok("已完成拉取、汇总并强制推送到远程仓库".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn link_app(app_id: String, git_path: String) -> Result<String, String> {
    let mut config = load_config();
    cleanup_legacy_skill_paths(&app_id)?;
    let skill_path = resolve_skill_path(&app_id, &config)?;
    let skill_dir = PathBuf::from(&skill_path);
    let backup_dir = get_backup_path(&skill_dir);
    let backup_path = backup_dir.to_string_lossy().to_string();
    let sync_dir = PathBuf::from(git_path.trim());

    if git_path.trim().is_empty() {
        return Err("Local sync directory is required".to_string());
    }

    let link_mode = detect_link_mode(&skill_dir, &app_id, &config);

    if backup_dir.exists() && !matches!(link_mode.as_deref(), Some("legacy" | "managed")) {
        return Err(format!(
            "Backup already exists. Unlink first: {}",
            backup_dir.to_string_lossy()
        ));
    }

    if sync_dir.exists() && !sync_dir.is_dir() {
        return Err(format!(
            "Local sync path must be a directory: {}",
            sync_dir.to_string_lossy()
        ));
    }

    let effective_enabled_skills = load_effective_enabled_skills(&config, &sync_dir)?;
    let enabled_entries = if effective_enabled_skills.contains_key(&app_id) {
        get_saved_enabled_entries(&app_id, &config, &sync_dir)?
    } else {
        let entries = list_sync_dir_entries(&sync_dir)?;
        save_enabled_entries_for_app(&mut config, &sync_dir, &app_id, entries.clone())?;
        entries
    };

    if skill_dir.exists() && !matches!(link_mode.as_deref(), Some("legacy" | "managed")) {
        fs::rename(&skill_dir, &backup_dir).map_err(|e| e.to_string())?;
    }

    let managed_dir = rebuild_managed_skill_dir(&app_id, &sync_dir, &enabled_entries)?;
    ensure_app_points_to_managed_dir(&skill_dir, &backup_dir, &managed_dir)?;

    Ok(backup_path)
}

#[tauri::command]
fn unlink_app(app_id: String) -> Result<(), String> {
    let config = load_config();
    let skill_path = resolve_skill_path(&app_id, &config)?;
    let skill_dir = PathBuf::from(&skill_path);
    let backup_dir = get_backup_path(&skill_dir);
    let managed_dir = resolve_managed_link_dir(&app_id);
    let linked_target = if skill_dir.exists() {
        fs::symlink_metadata(&skill_dir)
            .ok()
            .filter(|metadata| metadata.file_type().is_symlink())
            .and_then(|_| resolve_link_target(&skill_dir))
    } else {
        None
    };

    if backup_dir.exists() && !backup_dir.is_file() {
        if managed_dir.exists() {
            merge_local_only_paths(&managed_dir, &backup_dir, true)?;
        } else if let Some(target) = linked_target.as_ref() {
            merge_local_only_paths(target, &backup_dir, true)?;
        }
    }

    if skill_dir.exists() {
        if let Ok(metadata) = fs::symlink_metadata(&skill_dir) {
            if metadata.file_type().is_symlink() {
                remove_path_if_exists(&skill_dir)?;
            }
        }
    }

    if backup_dir.exists() {
        fs::rename(&backup_dir, &skill_dir).map_err(|e| e.to_string())?;
    }

    cleanup_legacy_skill_paths(&app_id)?;
    remove_path_if_exists(&managed_dir)?;

    Ok(())
}

#[tauri::command]
fn select_folder() -> Result<String, String> {
    Ok(String::new())
}

#[tauri::command]
fn save_git_path(path: String) -> Result<(), String> {
    let mut config = load_config();
    config.git_path = Some(path);
    save_config(&config)?;
    rebuild_managed_links_for_all_apps(&config)
}

#[cfg(target_os = "macos")]
fn macos_protected_folder_key(path: &Path) -> Option<&'static str> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let targets = [
        ("documents", home.join("Documents")),
        ("desktop", home.join("Desktop")),
        ("downloads", home.join("Downloads")),
    ];

    targets
        .iter()
        .find_map(|(key, target)| path.starts_with(target).then_some(*key))
}

fn map_directory_access_error(path: &Path, error: &std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        #[cfg(target_os = "macos")]
        if let Some(folder_key) = macos_protected_folder_key(path) {
            return format!("permission_denied:{}", folder_key);
        }
    }

    error.to_string()
}

fn probe_directory_access(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| map_directory_access_error(path, &error))?;
    fs::read_dir(path).map_err(|error| map_directory_access_error(path, &error))?;

    let probe_path = path.join(format!(".skillbox-access-probe-{}", std::process::id()));
    fs::write(&probe_path, b"skillbox-access-probe")
        .map_err(|error| map_directory_access_error(path, &error))?;
    fs::remove_file(&probe_path).map_err(|error| map_directory_access_error(path, &error))?;

    Ok(())
}

#[tauri::command]
fn probe_git_directory_access(path: String) -> Result<(), String> {
    let normalized_path = path.trim();
    if normalized_path.is_empty() {
        return Err("invalid_path".to_string());
    }

    probe_directory_access(Path::new(normalized_path))
}

fn normalize_version(value: &str) -> Option<Vec<u64>> {
    let core = value
        .trim()
        .trim_start_matches('v')
        .split(['-', '+'])
        .next()
        .unwrap_or_default();

    let mut parts = Vec::new();

    for segment in core.split('.') {
        if segment.is_empty() {
            return None;
        }

        parts.push(segment.parse::<u64>().ok()?);
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

fn is_version_newer(current: &str, latest: &str) -> bool {
    let Some(mut current_parts) = normalize_version(current) else {
        return current.trim() != latest.trim();
    };
    let Some(mut latest_parts) = normalize_version(latest) else {
        return current.trim() != latest.trim();
    };

    let max_len = current_parts.len().max(latest_parts.len());
    current_parts.resize(max_len, 0);
    latest_parts.resize(max_len, 0);

    latest_parts > current_parts
}

fn release_api_url() -> String {
    format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPOSITORY
    )
}

fn create_update_client(current_version: &str) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(format!("SkillBox/{}", current_version))
        .build()
        .map_err(|error| format!("Failed to create update client: {}", error))
}

async fn fetch_latest_release(
    client: &reqwest::Client,
    current_version: &str,
) -> Result<GitHubRelease, String> {
    let response = client
        .get(release_api_url())
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| format!("Failed to request latest release: {}", error))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("GitHub Releases 里还没有正式版本。".to_string());
    }

    if !response.status().is_success() {
        return Err(format!(
            "GitHub release check failed with status {}",
            response.status()
        ));
    }

    response.json::<GitHubRelease>().await.map_err(|error| {
        format!(
            "Failed to parse GitHub release response for {}: {}",
            current_version, error
        )
    })
}

fn score_release_asset(asset_name: &str) -> Option<i32> {
    let lower = asset_name.to_ascii_lowercase();

    #[cfg(target_os = "macos")]
    {
        if !(lower.ends_with(".dmg") || lower.ends_with(".tar.gz")) {
            return None;
        }

        let mut score = if lower.ends_with(".dmg") { 100 } else { 80 };

        #[cfg(target_arch = "aarch64")]
        {
            if lower.contains("aarch64") || lower.contains("arm64") {
                score += 30;
            } else if lower.contains("x64") || lower.contains("x86_64") || lower.contains("intel") {
                score -= 10;
            }
        }

        #[cfg(target_arch = "x86_64")]
        {
            if lower.contains("x64") || lower.contains("x86_64") || lower.contains("intel") {
                score += 30;
            } else if lower.contains("aarch64") || lower.contains("arm64") {
                score -= 10;
            }
        }

        return Some(score);
    }

    #[cfg(target_os = "windows")]
    {
        if !(lower.ends_with("-setup.exe") || lower.ends_with(".msi") || lower.ends_with(".exe")) {
            return None;
        }

        let mut score = if lower.ends_with("-setup.exe") {
            100
        } else if lower.ends_with(".msi") {
            90
        } else {
            80
        };

        #[cfg(target_arch = "x86_64")]
        {
            if lower.contains("x64") || lower.contains("x86_64") {
                score += 20;
            }
        }

        return Some(score);
    }

    #[cfg(target_os = "linux")]
    {
        if !(lower.ends_with(".appimage")
            || lower.ends_with(".deb")
            || lower.ends_with(".rpm")
            || lower.ends_with(".tar.gz"))
        {
            return None;
        }

        let score = if lower.ends_with(".appimage") {
            100
        } else if lower.ends_with(".deb") {
            90
        } else if lower.ends_with(".rpm") {
            80
        } else {
            70
        };

        return Some(score);
    }

    #[allow(unreachable_code)]
    None
}

fn select_release_asset(release: &GitHubRelease) -> Option<GitHubReleaseAsset> {
    release
        .assets
        .iter()
        .filter_map(|asset| score_release_asset(&asset.name).map(|score| (score, asset.clone())))
        .max_by_key(|(score, _)| *score)
        .map(|(_, asset)| asset)
}

fn update_download_dir() -> PathBuf {
    dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Downloads")))
        .unwrap_or_else(std::env::temp_dir)
        .join("SkillBox Updates")
}

fn emit_update_download_progress(
    window: &tauri::Window,
    file_name: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    status: &str,
) {
    let percentage = total_bytes
        .filter(|total| *total > 0)
        .map(|total| ((downloaded_bytes as f64 / total as f64) * 100.0).clamp(0.0, 100.0))
        .unwrap_or(0.0);

    let _ = window.emit(
        UPDATE_DOWNLOAD_PROGRESS_EVENT,
        UpdateDownloadProgressPayload {
            file_name: file_name.to_string(),
            downloaded_bytes,
            total_bytes,
            percentage,
            status: status.to_string(),
        },
    );
}

#[tauri::command]
async fn check_updates() -> Result<UpdateCheckResult, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let release_url = format!("{}/releases", GITHUB_REPOSITORY_URL);
    let client = create_update_client(&current_version)?;
    let release = match fetch_latest_release(&client, &current_version).await {
        Ok(release) => release,
        Err(error) if error == "GitHub Releases 里还没有正式版本。" => {
            return Ok(UpdateCheckResult {
                current_version,
                latest_version: None,
                update_available: false,
                release_url,
                release_name: None,
                published_at: None,
                notes: Some(error),
            });
        }
        Err(error) => return Err(error),
    };
    let latest_version = release.tag_name.trim().trim_start_matches('v').to_string();

    Ok(UpdateCheckResult {
        current_version: current_version.clone(),
        latest_version: Some(latest_version.clone()),
        update_available: is_version_newer(&current_version, &latest_version),
        release_url: release.html_url,
        release_name: release.name,
        published_at: release.published_at,
        notes: release.body.map(|value| value.trim().to_string()),
    })
}

#[tauri::command]
async fn download_update(window: tauri::Window) -> Result<DownloadUpdateResult, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let client = create_update_client(&current_version)?;
    let release = fetch_latest_release(&client, &current_version).await?;
    let latest_version = release.tag_name.trim().trim_start_matches('v').to_string();

    if !is_version_newer(&current_version, &latest_version) {
        return Err("当前已经是最新版本，无需下载更新。".to_string());
    }

    let asset = select_release_asset(&release).ok_or_else(|| {
        "当前平台暂时没有可下载的安装包，请前往 Releases 页面手动下载。".to_string()
    })?;

    let download_dir = update_download_dir();
    tokio_fs::create_dir_all(&download_dir)
        .await
        .map_err(|error| format!("Failed to create update download directory: {}", error))?;

    let target_path = download_dir.join(&asset.name);
    emit_update_download_progress(&window, &asset.name, 0, None, "preparing");

    // Check if the existing file matches the expected asset
    if target_path.exists() {
        // Verify the file name contains the latest version to ensure it's not a cached old version
        if asset.name.contains(&latest_version) {
            let existing_size = fs::metadata(&target_path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            emit_update_download_progress(
                &window,
                &asset.name,
                existing_size,
                Some(existing_size),
                "completed",
            );
            return Ok(DownloadUpdateResult {
                version: latest_version,
                file_name: asset.name,
                file_path: target_path.to_string_lossy().to_string(),
                release_url: release.html_url,
            });
        } else {
            // Old cached file, delete it and download fresh
            let _ = fs::remove_file(&target_path);
        }
    }

    let response = client
        .get(&asset.browser_download_url)
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .send()
        .await
        .map_err(|error| format!("Failed to download update asset: {}", error))?;

    if !response.status().is_success() {
        return Err(format!(
            "Update asset download failed with status {}",
            response.status()
        ));
    }

    let total_bytes = response.content_length();
    let temp_path = download_dir.join(format!("{}.part", &asset.name));
    let mut file = tokio_fs::File::create(&temp_path)
        .await
        .map_err(|error| format!("Failed to create temporary update file: {}", error))?;
    let mut downloaded_bytes = 0u64;
    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result
            .map_err(|error| format!("Failed to read update asset stream: {}", error))?;
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("Failed to write update asset: {}", error))?;
        downloaded_bytes += chunk.len() as u64;
        emit_update_download_progress(
            &window,
            &asset.name,
            downloaded_bytes,
            total_bytes,
            "downloading",
        );
    }

    file.flush()
        .await
        .map_err(|error| format!("Failed to flush update asset to disk: {}", error))?;
    drop(file);

    tokio_fs::rename(&temp_path, &target_path)
        .await
        .map_err(|error| format!("Failed to finalize update asset: {}", error))?;

    let final_size = fs::metadata(&target_path)
        .map(|metadata| metadata.len())
        .unwrap_or(downloaded_bytes);
    emit_update_download_progress(
        &window,
        &asset.name,
        final_size,
        total_bytes.or(Some(final_size)),
        "completed",
    );

    Ok(DownloadUpdateResult {
        version: latest_version,
        file_name: asset.name,
        file_path: target_path.to_string_lossy().to_string(),
        release_url: release.html_url,
    })
}

#[tauri::command]
fn open_downloaded_update(path: String) -> Result<(), String> {
    let target = PathBuf::from(&path);
    if !target.exists() {
        return Err(format!("Update installer does not exist: {}", path));
    }

    open_system_target(&target)
}

#[tauri::command]
fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
async fn figma_get_file(
    file_key: String,
    api_key: Option<String>,
) -> Result<FigmaFileData, String> {
    let key = api_key.ok_or("Figma API key is required")?;
    let client = FigmaClient::new(key);
    client.get_file(&file_key).await
}

#[tauri::command]
async fn figma_get_file_info(
    file_key: String,
    api_key: Option<String>,
) -> Result<FigmaFile, String> {
    let key = api_key.ok_or("Figma API key is required")?;
    let client = FigmaClient::new(key);
    client.get_file_info(&file_key).await
}

#[tauri::command]
async fn figma_get_images(
    file_key: String,
    node_ids: Vec<String>,
    api_key: Option<String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let key = api_key.ok_or("Figma API key is required")?;
    let client = FigmaClient::new(key);
    client.get_images(&file_key, &node_ids).await
}

#[tauri::command]
async fn figma_get_comments(
    file_key: String,
    api_key: Option<String>,
) -> Result<Vec<FigmaComment>, String> {
    let key = api_key.ok_or("Figma API key is required")?;
    let client = FigmaClient::new(key);
    client.get_comments(&file_key).await
}

#[tauri::command]
fn figma_extract_tokens(file_data: FigmaFileData) -> Result<Vec<DesignToken>, String> {
    let tokens = extract_design_tokens(&file_data.document);
    Ok(tokens)
}

#[tauri::command]
fn figma_extract_css(file_data: FigmaFileData) -> Result<String, String> {
    let css = extract_css_from_node(&file_data.document);
    Ok(css)
}

#[tauri::command]
fn figma_find_nodes(
    file_data: FigmaFileData,
    node_type: String,
) -> Result<Vec<figma::FigmaNode>, String> {
    let nodes = find_nodes_by_type(&file_data.document, &node_type);
    Ok(nodes)
}

#[tauri::command]
fn figma_find_nodes_by_name(
    file_data: FigmaFileData,
    name_pattern: String,
) -> Result<Vec<figma::FigmaNode>, String> {
    let nodes = find_nodes_by_name(&file_data.document, &name_pattern);
    Ok(nodes)
}

#[tauri::command]
fn scan_git_path_skills(path: String) -> Result<Vec<SkillFile>, String> {
    collect_skill_entries(Path::new(&path))
}

#[tauri::command]
fn save_figma_api_key(api_key: String) -> Result<(), String> {
    let mut config = load_config();
    config.figma_api_key = Some(api_key);
    save_config(&config)
}

#[tauri::command]
fn get_figma_api_key() -> Result<Option<String>, String> {
    let config = load_config();
    Ok(config.figma_api_key)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            scan_apps,
            scan_skills,
            scan_git_path_skills,
            search_skill_market,
            get_recommended_market_skills,
            get_market_skill_detail,
            install_skill_market,
            get_app_enabled_skills,
            save_app_enabled_skills,
            rename_skill,
            delete_skill,
            sync_to_git,
            get_git_config,
            save_git_config,
            git_push,
            git_pull,
            git_sync,
            link_app,
            unlink_app,
            select_folder,
            save_git_path,
            probe_git_directory_access,
            set_custom_path,
            open_path_in_file_manager,
            launch_app,
            add_custom_app,
            check_updates,
            download_update,
            open_downloaded_update,
            get_version,
            figma_get_file,
            figma_get_file_info,
            figma_get_images,
            figma_get_comments,
            figma_extract_tokens,
            figma_extract_css,
            figma_find_nodes,
            figma_find_nodes_by_name,
            save_figma_api_key,
            get_figma_api_key
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
