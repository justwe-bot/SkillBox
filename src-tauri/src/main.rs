#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod figma;

use futures_util::StreamExt;
use figma::{
    extract_css_from_node, extract_design_tokens, find_nodes_by_name, find_nodes_by_type,
    DesignToken, FigmaClient, FigmaComment, FigmaFile, FigmaFileData,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs as tokio_fs;
use tokio::io::AsyncWriteExt;
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SkillApp {
    id: String,
    name: String,
    path: String,
    icon: String,
    skill_count: usize,
    is_linked: bool,
    is_installed: bool,
    is_custom: bool,
    backup_path: Option<String>,
    custom_path: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    git_path: Option<String>,
    #[serde(default = "default_git_config")]
    git_config: GitSyncConfig,
    custom_paths: std::collections::HashMap<String, String>,
    figma_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitSyncConfig {
    #[serde(default)]
    repo_url: String,
    #[serde(default)]
    username: String,
    #[serde(default = "default_git_branch")]
    branch: String,
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

const SYNC_MANIFEST_FILE: &str = ".skillbox-sync.json";
const GITHUB_REPOSITORY: &str = "justwe-bot/SkillBox";
const GITHUB_REPOSITORY_URL: &str = "https://github.com/justwe-bot/SkillBox";
const UPDATE_DOWNLOAD_PROGRESS_EVENT: &str = "skillbox://update-download-progress";

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
            vec![home.join(".openclaw/skills")],
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
            ],
            vec![
                PathBuf::from("/Applications/CodeBuddy.app"),
                app_support.join("CodeBuddy"),
                home.join(".codebuddy"),
                home.join("CodeBuddy"),
            ],
        ),
        known_app(
            "copilot",
            "GitHub Copilot",
            "🧠",
            vec![
                home.join(".copilot/copilot-instructions.md"),
                home.join(".github/copilot-instructions.md"),
                home.join(".github/instructions"),
            ],
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
            "aider",
            "Aider",
            "🔧",
            vec![home.join(".aider/skills")],
            vec![home.join(".aider")],
        ),
        known_app(
            "continue",
            "Continue",
            "▶️",
            vec![
                home.join(".continue/rules"),
                home.join(".continue/prompts"),
                home.join(".continue/checks"),
                home.join(".continue/skills"),
            ],
            vec![home.join(".continue")],
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
            vec![home.join(".openclaw/skills")],
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
            ],
            vec![
                app_data.join("CodeBuddy"),
                home.join(".codebuddy"),
                home.join("CodeBuddy"),
            ],
        ),
        known_app(
            "copilot",
            "GitHub Copilot",
            "🧠",
            vec![
                home.join(".copilot/copilot-instructions.md"),
                home.join(".github/copilot-instructions.md"),
                home.join(".github/instructions"),
            ],
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
            "aider",
            "Aider",
            "🔧",
            vec![home.join(".aider/skills")],
            vec![home.join(".aider")],
        ),
        known_app(
            "continue",
            "Continue",
            "▶️",
            vec![
                home.join(".continue/rules"),
                home.join(".continue/prompts"),
                home.join(".continue/checks"),
                home.join(".continue/skills"),
            ],
            vec![home.join(".continue")],
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
            vec![home.join(".openclaw/skills")],
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
            ],
            vec![
                config_dir.join("CodeBuddy"),
                home.join(".codebuddy"),
                home.join("CodeBuddy"),
            ],
        ),
        known_app(
            "copilot",
            "GitHub Copilot",
            "🧠",
            vec![
                home.join(".copilot/copilot-instructions.md"),
                home.join(".github/copilot-instructions.md"),
                home.join(".github/instructions"),
            ],
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
            "aider",
            "Aider",
            "🔧",
            vec![home.join(".aider/skills")],
            vec![home.join(".aider")],
        ),
        known_app(
            "continue",
            "Continue",
            "▶️",
            vec![
                home.join(".continue/rules"),
                home.join(".continue/prompts"),
                home.join(".continue/checks"),
                home.join(".continue/skills"),
            ],
            vec![home.join(".continue")],
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
    let name = entry.file_name().to_string_lossy();
    !matches!(
        name.as_ref(),
        ".git" | "node_modules" | "target" | "__pycache__" | ".DS_Store"
    )
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
        for line in &mut lines {
            if line.trim() == "---" {
                break;
            }

            if let Some((key, value)) = line.split_once(':') {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                match key.trim() {
                    "name" if !value.is_empty() => name = Some(value.to_string()),
                    "description" if !value.is_empty() => description = Some(value.to_string()),
                    _ => {}
                }
            }
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
        username: String::new(),
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
        custom_paths: std::collections::HashMap::new(),
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

fn run_git(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git {}: {}", args.join(" "), e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if stderr.is_empty() { stdout } else { stderr };
        Err(format!("git {} failed: {}", args.join(" "), message))
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

fn save_sync_manifest(repo_path: &Path, entries: &[String]) -> Result<(), String> {
    let manifest_path = repo_path.join(SYNC_MANIFEST_FILE);
    let content = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    fs::write(manifest_path, content).map_err(|e| e.to_string())
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


fn is_directory_empty(path: &Path) -> Result<bool, String> {
    Ok(fs::read_dir(path)
        .map_err(|e| e.to_string())?
        .next()
        .transpose()
        .map_err(|e| e.to_string())?
        .is_none())
}

fn ensure_repo_initialized(repo_path: &Path, git_config: &GitSyncConfig) -> Result<(), String> {
    if !repo_path.exists() {
        fs::create_dir_all(repo_path).map_err(|e| e.to_string())?;
    }

    let git_dir = repo_path.join(".git");
    if !git_dir.exists() {
        let repo_url = git_config.repo_url.trim();
        let branch = git_config.branch.trim();

        if !repo_url.is_empty() && is_directory_empty(repo_path)? {
            let parent = repo_path
                .parent()
                .ok_or_else(|| "Unable to resolve repository parent directory".to_string())?;
            let repo_name = repo_path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| "Unable to resolve repository directory name".to_string())?;

            let mut clone_args = vec!["clone"];
            if !branch.is_empty() {
                clone_args.push("--branch");
                clone_args.push(branch);
            }
            clone_args.push(repo_url);
            clone_args.push(repo_name);

            if let Err(error) = run_git(parent, &clone_args) {
                let missing_branch = error.contains("Remote branch") && error.contains("not found");

                if missing_branch {
                    run_git(parent, &["clone", repo_url, repo_name])?;
                } else {
                    return Err(error);
                }
            }
        } else {
            if !branch.is_empty() {
                let _ = run_git(repo_path, &["init", "-b", branch]);
            }

            if !git_dir.exists() {
                run_git(repo_path, &["init"])?;
            }
        }
    }

    if !repo_path.join(".git").exists() {
        return Err("Local sync directory is not a git repository".to_string());
    }

    if !git_config.username.trim().is_empty() {
        let _ = run_git(
            repo_path,
            &["config", "user.name", git_config.username.trim()],
        );
    }

    if !git_config.repo_url.trim().is_empty() {
        let remote_exists = run_git(repo_path, &["remote", "get-url", "origin"]).is_ok();
        if remote_exists {
            run_git(
                repo_path,
                &["remote", "set-url", "origin", git_config.repo_url.trim()],
            )?;
        } else {
            run_git(
                repo_path,
                &["remote", "add", "origin", git_config.repo_url.trim()],
            )?;
        }
    }

    // Only force-switch branch when there are no commits yet; if the repo
    // already has commits (i.e. local skills exist) we must NOT run
    // `checkout -B` here because it would fail when untracked files share
    // names with remote files.  Branch alignment for repos that already have
    // commits is handled inside git_pull / git_push instead.
    if !git_config.branch.trim().is_empty() && !repo_has_any_commit(repo_path)? {
        let _ = run_git(repo_path, &["checkout", "-B", git_config.branch.trim()]);
    }

    Ok(())
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

    run_git(repo_path, &["commit", "-m", "Sync AI skills"])?;
    Ok(true)
}

fn repo_has_local_changes(repo_path: &Path) -> Result<bool, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git status --porcelain: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "git status --porcelain failed".to_string()
        } else {
            format!("git status --porcelain failed: {}", stderr)
        });
    }

    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

fn repo_has_any_commit(repo_path: &Path) -> Result<bool, String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git rev-parse HEAD: {}", e))?;

    Ok(output.status.success())
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

    let target = app
        .install_markers
        .iter()
        .find(|path| {
            path.exists()
                && (path
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(|value| value.eq_ignore_ascii_case("app"))
                    .unwrap_or(false)
                    || path.is_file())
        })
        .cloned()
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
        let is_installed = custom_path
            .as_ref()
            .map(|value| PathBuf::from(value).exists())
            .unwrap_or(false)
            || app.skill_paths.iter().any(|value| value.exists())
            || app.install_markers.iter().any(|value| value.exists())
            || backup_path.is_some();

        apps.push(SkillApp {
            id: app.id.clone(),
            name: app.name,
            path,
            icon: app.icon,
            skill_count: 0,
            is_linked,
            is_installed,
            is_custom: false,
            backup_path,
            custom_path,
        });
    }

    for (id, custom_path) in &config.custom_paths {
        if !apps.iter().any(|a| a.id == *id) {
            let is_installed = PathBuf::from(custom_path).exists();
            let (is_linked, backup_path) = check_link_status(custom_path);

            apps.push(SkillApp {
                id: id.clone(),
                name: capitalize_first(id),
                path: custom_path.clone(),
                icon: "📁".to_string(),
                skill_count: 0,
                is_linked,
                is_installed,
                is_custom: true,
                backup_path,
                custom_path: Some(custom_path.clone()),
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
        return fs::remove_file(&target)
            .or_else(|_| fs::remove_dir(&target))
            .map_err(|e| e.to_string());
    }

    if metadata.is_dir() {
        fs::remove_dir_all(&target).map_err(|e| e.to_string())
    } else {
        fs::remove_file(&target).map_err(|e| e.to_string())
    }
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

#[tauri::command]
fn sync_to_git(repo_path: String) -> Result<(), String> {
    let repo = PathBuf::from(&repo_path);
    if !repo.exists() {
        fs::create_dir_all(&repo).map_err(|e| e.to_string())?;
    }

    let _config = load_config();
    let apps = scan_apps().map_err(|e| e)?;
    let repo_real = repo.canonicalize().unwrap_or(repo.clone());

    let mut written_entries = Vec::new();
    let mut used_names = std::collections::HashSet::new();
    let mut seen_skill_dirs = std::collections::HashSet::new();

    // 收集同步目录中已有的技能名称
    if let Ok(existing_entries) = fs::read_dir(&repo) {
        for entry in existing_entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name != ".git" && name != SYNC_MANIFEST_FILE {
                used_names.insert(name);
            }
        }
    }

    for app in apps.0 {
        if app.is_installed || app.is_linked {
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
                            written_entries.push(base_name);
                            continue;
                        }
                    }

                    let flat_name = make_flat_skill_name(&skill, &app.id, &mut used_names);

                    let target = repo.join(&flat_name);
                    let target_real = target.canonicalize().unwrap_or(target.clone());
                    if source_real == target_real {
                        continue;
                    }

                    copy_path_recursive(&source, &target)?;
                    written_entries.push(flat_name);
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
    save_sync_manifest(&repo, &written_entries)?;

    Ok(())
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
        username: config.username.trim().to_string(),
        branch: if config.branch.trim().is_empty() {
            default_git_branch()
        } else {
            config.branch.trim().to_string()
        },
    };
    save_config(&app_config)
}

#[tauri::command]
fn git_push(repo_path: String) -> Result<(), String> {
    let app_config = load_config();
    if app_config.git_config.repo_url.trim().is_empty() {
        return Err("请先保存仓库地址".to_string());
    }

    let repo = PathBuf::from(&repo_path);
    ensure_repo_initialized(&repo, &app_config.git_config)?;
    sync_to_git(repo_path.clone())?;
    let _ = commit_repo_changes(&repo)?;
    run_git(
        &repo,
        &["push", "-u", "origin", app_config.git_config.branch.trim()],
    )?;
    Ok(())
}

#[tauri::command]
fn git_pull(repo_path: String) -> Result<String, String> {
    let app_config = load_config();
    if app_config.git_config.repo_url.trim().is_empty() {
        return Err("请先保存仓库地址".to_string());
    }

    let repo = PathBuf::from(&repo_path);
    ensure_repo_initialized(&repo, &app_config.git_config)?;
    
    let has_commit = repo_has_any_commit(&repo)?;

    if !has_commit {
        let branch = app_config.git_config.branch.trim().to_string();

        // Commit whatever local skills already exist so they are preserved.
        run_git(&repo, &["add", "."])?;
        let _ = commit_repo_changes(&repo)?;

        // Fetch remote content.
        run_git(&repo, &["fetch", "origin", &branch])?;

        let fetch_head = repo.join(".git/FETCH_HEAD");
        if fetch_head.exists() {
            // Merge remote history into the local initial commit so that
            // local skills (5) and remote skills (10) are combined (15).
            // --allow-unrelated-histories is required because the two trees
            // have no common ancestor yet.
            let merge_result = run_git(
                &repo,
                &[
                    "merge",
                    "--allow-unrelated-histories",
                    "--no-edit",
                    "FETCH_HEAD",
                ],
            );

            if let Err(e) = merge_result {
                // Surface a clear message if there are genuine conflicts.
                return Err(format!("合并远程内容时发生冲突，请手动解决后重试: {}", e));
            }

            // Align the local branch name with the configured branch.
            let _ = run_git(&repo, &["branch", "-M", &branch]);
        }

        return Ok("已将本地 skills 与远程仓库合并".to_string());
    }
    
    let branch = app_config.git_config.branch.trim().to_string();

    // Commit any local changes first so nothing is lost.
    // We intentionally avoid `git stash` here because it fails when the
    // working tree contains symlinks or files with unusual permissions
    // (error: "could not write index").
    let had_local_changes = repo_has_local_changes(&repo)?;
    if had_local_changes {
        run_git(&repo, &["add", "."])?;
        let _ = commit_repo_changes(&repo)?;
    }

    // Fetch then merge so that local commits and remote commits are combined.
    // --allow-unrelated-histories handles the case where the two trees have
    // diverged (e.g. local was initialised independently of the remote).
    run_git(&repo, &["fetch", "origin", &branch])?;

    let merge_result = run_git(
        &repo,
        &[
            "merge",
            "--allow-unrelated-histories",
            "--no-edit",
            &format!("origin/{}", branch),
        ],
    );

    if let Err(e) = merge_result {
        return Err(format!("合并远程内容时发生冲突，请手动解决后重试: {}", e));
    }

    Ok("已从远程仓库拉取最新内容".to_string())
}

#[tauri::command]
fn git_sync(repo_path: String) -> Result<String, String> {
    let app_config = load_config();
    if app_config.git_config.repo_url.trim().is_empty() {
        return Err("请先保存仓库地址".to_string());
    }

    let repo = PathBuf::from(&repo_path);
    ensure_repo_initialized(&repo, &app_config.git_config)?;

    let branch = app_config.git_config.branch.trim().to_string();
    let has_commit = repo_has_any_commit(&repo)?;

    if !has_commit {
        // No local commits yet — commit existing files first, then merge remote.
        run_git(&repo, &["add", "."])?;
        let _ = commit_repo_changes(&repo)?;

        run_git(&repo, &["fetch", "origin", &branch])?;

        let fetch_head = repo.join(".git/FETCH_HEAD");
        if fetch_head.exists() {
            let _ = run_git(
                &repo,
                &["merge", "--allow-unrelated-histories", "--no-edit", "FETCH_HEAD"],
            );
            let _ = run_git(&repo, &["branch", "-M", &branch]);
        }
    } else {
        // Commit any local changes before fetching so stash is never needed.
        if repo_has_local_changes(&repo)? {
            run_git(&repo, &["add", "."])?;
            let _ = commit_repo_changes(&repo)?;
        }
        run_git(&repo, &["fetch", "origin", &branch])?;
        let _ = run_git(
            &repo,
            &["merge", "--allow-unrelated-histories", "--no-edit", &format!("origin/{}", branch)],
        );
    }
    
    sync_to_git(repo_path.clone())?;
    let _ = commit_repo_changes(&repo)?;
    run_git(
        &repo,
        &["push", "-u", "origin", &branch],
    )?;
    Ok("已完成拉取、同步并推送到远程仓库".to_string())
}

#[tauri::command]
fn link_app(app_id: String, git_path: String) -> Result<String, String> {
    let config = load_config();
    let skill_path = resolve_skill_path(&app_id, &config)?;
    let skill_dir = PathBuf::from(&skill_path);
    let backup_dir = get_backup_path(&skill_dir);
    let backup_path = backup_dir.to_string_lossy().to_string();
    let sync_dir = PathBuf::from(git_path.trim());

    if git_path.trim().is_empty() {
        return Err("Local sync directory is required".to_string());
    }

    if backup_dir.exists() {
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

    if skill_dir.exists() {
        fs::rename(&skill_dir, &backup_dir).map_err(|e| e.to_string())?;
    }

    if let Some(parent) = skill_dir.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    fs::create_dir_all(&sync_dir).map_err(|e| e.to_string())?;

    #[cfg(unix)]
    std::os::unix::fs::symlink(&sync_dir, &skill_dir).map_err(|e| e.to_string())?;

    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&sync_dir, &skill_dir).map_err(|e| e.to_string())?;

    Ok(backup_path)
}

#[tauri::command]
fn unlink_app(app_id: String) -> Result<(), String> {
    let config = load_config();
    let skill_path = resolve_skill_path(&app_id, &config)?;
    let skill_dir = PathBuf::from(&skill_path);
    let backup_dir = get_backup_path(&skill_dir);

    if skill_dir.exists() {
        if let Ok(metadata) = fs::symlink_metadata(&skill_dir) {
            if metadata.file_type().is_symlink() {
                fs::remove_file(&skill_dir)
                    .or_else(|_| fs::remove_dir(&skill_dir))
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    if backup_dir.exists() {
        fs::rename(&backup_dir, &skill_dir).map_err(|e| e.to_string())?;
    }

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
    save_config(&config)
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

    response
        .json::<GitHubRelease>()
        .await
        .map_err(|error| {
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
    if target_path.exists() {
        let existing_size = fs::metadata(&target_path).map(|metadata| metadata.len()).unwrap_or(0);
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
        let chunk = chunk_result.map_err(|error| format!("Failed to read update asset stream: {}", error))?;
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

    let final_size = fs::metadata(&target_path).map(|metadata| metadata.len()).unwrap_or(downloaded_bytes);
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
