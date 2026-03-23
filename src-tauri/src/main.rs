#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod figma;

use sha2::{Digest, Sha256};
use figma::{FigmaClient, FigmaFileData, DesignToken, FigmaComment, FigmaFile, extract_design_tokens, extract_css_from_node, find_nodes_by_type, find_nodes_by_name};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
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
    custom_paths: std::collections::HashMap<String, String>,
    figma_api_key: Option<String>,
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

fn build_known_apps() -> Vec<KnownApp> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut apps = Vec::new();

    #[cfg(target_os = "macos")]
    {
        let app_support = home.join("Library/Application Support");

        apps.push(KnownApp {
            id: "codex".to_string(),
            name: "Codex".to_string(),
            icon: "📦".to_string(),
            skill_paths: vec![home.join(".codex/skills")],
            install_markers: vec![
                PathBuf::from("/Applications/Codex.app"),
                home.join(".codex"),
                home.join(".codex/config.toml"),
            ],
        });
        apps.push(KnownApp {
            id: "openclaw".to_string(),
            name: "Openclaw".to_string(),
            icon: "🦀".to_string(),
            skill_paths: vec![home.join(".openclaw/skills")],
            install_markers: vec![home.join(".openclaw")],
        });
        apps.push(KnownApp {
            id: "opencode".to_string(),
            name: "Opencode".to_string(),
            icon: "💻".to_string(),
            skill_paths: vec![home.join(".config/opencode/skills")],
            install_markers: vec![home.join(".config/opencode")],
        });
        apps.push(KnownApp {
            id: "cline".to_string(),
            name: "Cline".to_string(),
            icon: "⚡".to_string(),
            skill_paths: vec![home.join(".cline/skills")],
            install_markers: vec![home.join(".cline")],
        });
        apps.push(KnownApp {
            id: "cursor".to_string(),
            name: "Cursor".to_string(),
            icon: "🎯".to_string(),
            skill_paths: vec![
                home.join(".cursor/skills"),
                app_support.join("Cursor/User/globalStorage/skills"),
            ],
            install_markers: vec![
                PathBuf::from("/Applications/Cursor.app"),
                app_support.join("Cursor"),
                home.join(".cursor"),
            ],
        });
        apps.push(KnownApp {
            id: "windsurf".to_string(),
            name: "Windsurf".to_string(),
            icon: "🌊".to_string(),
            skill_paths: vec![
                home.join(".windsurf/skills"),
                app_support.join("Windsurf/User/globalStorage/skills"),
            ],
            install_markers: vec![
                PathBuf::from("/Applications/Windsurf.app"),
                app_support.join("Windsurf"),
                home.join(".windsurf"),
            ],
        });
        apps.push(KnownApp {
            id: "claude".to_string(),
            name: "Claude".to_string(),
            icon: "🤖".to_string(),
            skill_paths: vec![
                home.join(".claude/skills"),
                app_support.join("Claude/claude_desktop_skills"),
            ],
            install_markers: vec![
                PathBuf::from("/Applications/Claude.app"),
                app_support.join("Claude"),
                home.join(".claude"),
            ],
        });
        apps.push(KnownApp {
            id: "aider".to_string(),
            name: "Aider".to_string(),
            icon: "🔧".to_string(),
            skill_paths: vec![home.join(".aider/skills")],
            install_markers: vec![home.join(".aider")],
        });
        apps.push(KnownApp {
            id: "continue".to_string(),
            name: "Continue".to_string(),
            icon: "▶️".to_string(),
            skill_paths: vec![home.join(".continue/skills")],
            install_markers: vec![home.join(".continue")],
        });
    }

    #[cfg(target_os = "windows")]
    {
        let app_data = PathBuf::from(std::env::var("APPDATA").unwrap_or_default());

        apps.push(KnownApp {
            id: "codex".to_string(),
            name: "Codex".to_string(),
            icon: "📦".to_string(),
            skill_paths: vec![home.join(".codex/skills")],
            install_markers: vec![home.join(".codex"), home.join(".codex/config.toml")],
        });
        apps.push(KnownApp {
            id: "openclaw".to_string(),
            name: "Openclaw".to_string(),
            icon: "🦀".to_string(),
            skill_paths: vec![home.join(".openclaw/skills")],
            install_markers: vec![home.join(".openclaw")],
        });
        apps.push(KnownApp {
            id: "opencode".to_string(),
            name: "Opencode".to_string(),
            icon: "💻".to_string(),
            skill_paths: vec![home.join(".config/opencode/skills")],
            install_markers: vec![home.join(".config/opencode")],
        });
        apps.push(KnownApp {
            id: "cline".to_string(),
            name: "Cline".to_string(),
            icon: "⚡".to_string(),
            skill_paths: vec![home.join(".cline/skills")],
            install_markers: vec![home.join(".cline")],
        });
        apps.push(KnownApp {
            id: "cursor".to_string(),
            name: "Cursor".to_string(),
            icon: "🎯".to_string(),
            skill_paths: vec![
                home.join(".cursor/skills"),
                app_data.join("Cursor/User/globalStorage/skills"),
            ],
            install_markers: vec![app_data.join("Cursor"), home.join(".cursor")],
        });
        apps.push(KnownApp {
            id: "windsurf".to_string(),
            name: "Windsurf".to_string(),
            icon: "🌊".to_string(),
            skill_paths: vec![
                home.join(".windsurf/skills"),
                app_data.join("Windsurf/User/globalStorage/skills"),
            ],
            install_markers: vec![app_data.join("Windsurf"), home.join(".windsurf")],
        });
        apps.push(KnownApp {
            id: "claude".to_string(),
            name: "Claude".to_string(),
            icon: "🤖".to_string(),
            skill_paths: vec![
                home.join(".claude/skills"),
                app_data.join("Claude/claude_desktop_skills"),
            ],
            install_markers: vec![app_data.join("Claude"), home.join(".claude")],
        });
        apps.push(KnownApp {
            id: "aider".to_string(),
            name: "Aider".to_string(),
            icon: "🔧".to_string(),
            skill_paths: vec![home.join(".aider/skills")],
            install_markers: vec![home.join(".aider")],
        });
        apps.push(KnownApp {
            id: "continue".to_string(),
            name: "Continue".to_string(),
            icon: "▶️".to_string(),
            skill_paths: vec![home.join(".continue/skills")],
            install_markers: vec![home.join(".continue")],
        });
    }

    #[cfg(target_os = "linux")]
    {
        let config_dir = home.join(".config");

        apps.push(KnownApp {
            id: "codex".to_string(),
            name: "Codex".to_string(),
            icon: "📦".to_string(),
            skill_paths: vec![home.join(".codex/skills")],
            install_markers: vec![home.join(".codex"), home.join(".codex/config.toml")],
        });
        apps.push(KnownApp {
            id: "openclaw".to_string(),
            name: "Openclaw".to_string(),
            icon: "🦀".to_string(),
            skill_paths: vec![home.join(".openclaw/skills")],
            install_markers: vec![home.join(".openclaw")],
        });
        apps.push(KnownApp {
            id: "opencode".to_string(),
            name: "Opencode".to_string(),
            icon: "💻".to_string(),
            skill_paths: vec![config_dir.join("opencode/skills")],
            install_markers: vec![config_dir.join("opencode")],
        });
        apps.push(KnownApp {
            id: "cline".to_string(),
            name: "Cline".to_string(),
            icon: "⚡".to_string(),
            skill_paths: vec![home.join(".cline/skills")],
            install_markers: vec![home.join(".cline")],
        });
        apps.push(KnownApp {
            id: "cursor".to_string(),
            name: "Cursor".to_string(),
            icon: "🎯".to_string(),
            skill_paths: vec![
                home.join(".cursor/skills"),
                config_dir.join("Cursor/User/globalStorage/skills"),
            ],
            install_markers: vec![config_dir.join("Cursor"), home.join(".cursor")],
        });
        apps.push(KnownApp {
            id: "windsurf".to_string(),
            name: "Windsurf".to_string(),
            icon: "🌊".to_string(),
            skill_paths: vec![
                home.join(".windsurf/skills"),
                config_dir.join("Windsurf/User/globalStorage/skills"),
            ],
            install_markers: vec![config_dir.join("Windsurf"), home.join(".windsurf")],
        });
        apps.push(KnownApp {
            id: "claude".to_string(),
            name: "Claude".to_string(),
            icon: "🤖".to_string(),
            skill_paths: vec![
                home.join(".claude/skills"),
                config_dir.join("claude/claude_desktop_skills"),
            ],
            install_markers: vec![config_dir.join("claude"), home.join(".claude")],
        });
        apps.push(KnownApp {
            id: "aider".to_string(),
            name: "Aider".to_string(),
            icon: "🔧".to_string(),
            skill_paths: vec![home.join(".aider/skills")],
            install_markers: vec![home.join(".aider")],
        });
        apps.push(KnownApp {
            id: "continue".to_string(),
            name: "Continue".to_string(),
            icon: "▶️".to_string(),
            skill_paths: vec![home.join(".continue/skills")],
            install_markers: vec![home.join(".continue")],
        });
    }

    apps
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
        hasher.update(path.file_name().and_then(|v| v.to_str()).unwrap_or_default().as_bytes());
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

    let modified = latest_modified
        .map(format_system_time)
        .unwrap_or_default();

    Ok((total_size, modified, file_count, format!("{:x}", hasher.finalize())))
}

fn collect_skill_entries(path: &Path) -> Result<Vec<SkillFile>, String> {
    if !path.exists() || !path.is_dir() {
        return Ok(vec![]);
    }

    let mut seen_paths = HashSet::new();
    let mut skills = Vec::new();

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
        let is_skill_markdown = file_name == "SKILL.md";
        let is_skill_file = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("skill"))
            .unwrap_or(false);

        if !is_skill_markdown && !is_skill_file {
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

fn check_link_status(path: &str) -> (bool, Option<String>) {
    let path_obj = PathBuf::from(path);
    if path_obj.exists() {
        if let Ok(metadata) = fs::symlink_metadata(&path_obj) {
            if metadata.file_type().is_symlink() {
                return (true, None);
            }
        }
    }

    let backup_path = format!("{}_backup", path);
    if PathBuf::from(&backup_path).exists() {
        return (false, Some(backup_path));
    }

    (false, None)
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
        let skill_count = collect_skill_entries(Path::new(&path))
            .map(|entries| entries.len())
            .unwrap_or(0);

        apps.push(SkillApp {
            id: app.id.clone(),
            name: app.name,
            path,
            icon: app.icon,
            skill_count,
            is_linked,
            is_installed,
            backup_path,
            custom_path,
        });
    }
    
    for (id, custom_path) in &config.custom_paths {
        if !apps.iter().any(|a| a.id == *id) {
            let is_installed = PathBuf::from(custom_path).exists();
            let (is_linked, backup_path) = check_link_status(custom_path);
            let skill_count = collect_skill_entries(Path::new(custom_path))
                .map(|entries| entries.len())
                .unwrap_or(0);
            
            apps.push(SkillApp {
                id: id.clone(),
                name: capitalize_first(id),
                path: custom_path.clone(),
                icon: "📁".to_string(),
                skill_count,
                is_linked,
                is_installed,
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
    
    for app in apps.0 {
        if app.is_installed || app.is_linked {
            let target_dir = repo.join(&app.id);
            fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
            
            let skill_dir = PathBuf::from(&app.path);
            if skill_dir.exists() && skill_dir.is_dir() {
                if let Ok(entries) = fs::read_dir(&skill_dir) {
                    for entry in entries.filter_map(Result::ok) {
                        let src = entry.path();
                        if src.is_file() {
                            let dst = target_dir.join(src.file_name().unwrap());
                            fs::copy(&src, &dst).map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}

#[tauri::command]
fn link_app(app_id: String, git_path: String) -> Result<String, String> {
    let config = load_config();
    let skill_path = resolve_skill_path(&app_id, &config)?;
    
    let skill_dir = PathBuf::from(&skill_path);
    let backup_path = format!("{}_backup", skill_path);
    let backup_dir = PathBuf::from(&backup_path);
    let git_skill_dir = PathBuf::from(&git_path).join(&app_id);
    
    if backup_dir.exists() {
        return Err("Backup already exists. Unlink first.".to_string());
    }
    
    if skill_dir.exists() {
        fs::rename(&skill_dir, &backup_dir).map_err(|e| e.to_string())?;
    }
    
    fs::create_dir_all(&git_skill_dir).map_err(|e| e.to_string())?;
    
    #[cfg(unix)]
    std::os::unix::fs::symlink(&git_skill_dir, &skill_dir).map_err(|e| e.to_string())?;
    
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&git_skill_dir, &skill_dir).map_err(|e| e.to_string())?;
    
    Ok(backup_path)
}

#[tauri::command]
fn unlink_app(app_id: String) -> Result<(), String> {
    let config = load_config();
    let skill_path = resolve_skill_path(&app_id, &config)?;
    
    let skill_dir = PathBuf::from(&skill_path);
    let backup_path = format!("{}_backup", skill_path);
    let backup_dir = PathBuf::from(&backup_path);
    
    if skill_dir.exists() {
        if let Ok(metadata) = fs::symlink_metadata(&skill_dir) {
            if metadata.file_type().is_symlink() {
                fs::remove_file(&skill_dir).or_else(|_| fs::remove_dir(&skill_dir)).map_err(|e| e.to_string())?;
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

#[tauri::command]
fn check_updates() -> Result<String, String> {
    Ok("You are using the latest version".to_string())
}

#[tauri::command]
fn get_version() -> String {
    "1.0.0".to_string()
}

#[tauri::command]
async fn figma_get_file(file_key: String, api_key: Option<String>) -> Result<FigmaFileData, String> {
    let key = api_key.ok_or("Figma API key is required")?;
    let client = FigmaClient::new(key);
    client.get_file(&file_key).await
}

#[tauri::command]
async fn figma_get_file_info(file_key: String, api_key: Option<String>) -> Result<FigmaFile, String> {
    let key = api_key.ok_or("Figma API key is required")?;
    let client = FigmaClient::new(key);
    client.get_file_info(&file_key).await
}

#[tauri::command]
async fn figma_get_images(file_key: String, node_ids: Vec<String>, api_key: Option<String>) -> Result<std::collections::HashMap<String, String>, String> {
    let key = api_key.ok_or("Figma API key is required")?;
    let client = FigmaClient::new(key);
    client.get_images(&file_key, &node_ids).await
}

#[tauri::command]
async fn figma_get_comments(file_key: String, api_key: Option<String>) -> Result<Vec<FigmaComment>, String> {
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
fn figma_find_nodes(file_data: FigmaFileData, node_type: String) -> Result<Vec<figma::FigmaNode>, String> {
    let nodes = find_nodes_by_type(&file_data.document, &node_type);
    Ok(nodes)
}

#[tauri::command]
fn figma_find_nodes_by_name(file_data: FigmaFileData, name_pattern: String) -> Result<Vec<figma::FigmaNode>, String> {
    let nodes = find_nodes_by_name(&file_data.document, &name_pattern);
    Ok(nodes)
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
            sync_to_git,
            link_app,
            unlink_app,
            select_folder,
            save_git_path,
            set_custom_path,
            add_custom_app,
            check_updates,
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
