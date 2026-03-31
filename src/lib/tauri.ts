import { invoke } from '@tauri-apps/api/tauri'
import type {
  AppEnabledSkillsState,
  AppRecord,
  BackendApp,
  BackendAppEnabledSkillsResponse,
  BackendSkillFile,
  DownloadUpdateResult,
  GitSyncConfig,
  ManagedSkillEntry,
  ScanAppsResponse,
  SkillRecord,
  UpdateCheckResult,
} from '../types'
import { LOCAL_SYNC_SOURCE } from './i18n'

interface SkillInventoryChunk {
  app: AppRecord
  files: BackendSkillFile[]
}

export const updateDownloadProgressEvent = 'skillbox://update-download-progress'

function mapApp(app: BackendApp): AppRecord {
  return {
    id: app.id,
    name: app.name,
    path: app.path,
    icon: app.icon,
    skillCount: app.skill_count,
    enabledSkillCount: app.enabled_skill_count,
    isLinked: app.is_linked,
    isInstalled: app.is_installed,
    isCustom: app.is_custom ?? false,
    backupPath: app.backup_path,
    customPath: app.custom_path,
    linkMode: app.link_mode,
  }
}

export async function scanApps() {
  const result = await invoke<ScanAppsResponse>('scan_apps')

  if (Array.isArray(result)) {
    return {
      apps: result[0].map(mapApp),
      gitPath: result[1] ?? '',
    }
  }

  return {
    apps: (result.apps ?? []).map(mapApp),
    gitPath: result.gitPath ?? result.git_path ?? '',
  }
}

export function scanGitPathSkills(path: string) {
  return invoke<BackendSkillFile[]>('scan_git_path_skills', { path })
}

export function scanSkills(appId: string) {
  return invoke<BackendSkillFile[]>('scan_skills', { appId })
}

export function renameSkill(skillPath: string, newName: string) {
  return invoke<string>('rename_skill', { skillPath, newName })
}

export function deleteSkill(skillPath: string) {
  return invoke<void>('delete_skill', { skillPath })
}

export function linkApp(appId: string, gitPath: string) {
  return invoke<string>('link_app', { appId, gitPath })
}

export function unlinkApp(appId: string) {
  return invoke<void>('unlink_app', { appId })
}

function mapManagedSkillEntry(skill: BackendAppEnabledSkillsResponse['skills'][number]): ManagedSkillEntry {
  return {
    id: `managed:${skill.entryName}`,
    entryName: skill.entryName,
    name: skill.name,
    description: skill.description || skill.path,
    path: skill.path,
    size: skill.size,
    modified: skill.modified,
    sources: [LOCAL_SYNC_SOURCE],
    conflicts: false,
    duplicateCount: 1,
    canonicalName: skill.canonical_name || skill.name.toLowerCase(),
    contentHashes: [skill.content_hash],
    fileCount: skill.file_count,
    enabled: skill.enabled,
  }
}

export async function getAppEnabledSkills(appId: string, gitPath: string): Promise<AppEnabledSkillsState> {
  const result = await invoke<BackendAppEnabledSkillsResponse>('get_app_enabled_skills', { appId, gitPath })
  return {
    appId: result.appId,
    linkMode: result.linkMode,
    enabledEntries: result.enabledEntries,
    skills: result.skills.map(mapManagedSkillEntry),
  }
}

export function saveAppEnabledSkills(appId: string, gitPath: string, enabledEntries: string[]) {
  return invoke<void>('save_app_enabled_skills', { appId, gitPath, enabledEntries })
}

export function syncToGit(repoPath: string) {
  return invoke<void>('sync_to_git', { repoPath })
}

export function saveGitPath(path: string) {
  return invoke<void>('save_git_path', { path })
}

export function probeGitDirectoryAccess(path: string) {
  return invoke<void>('probe_git_directory_access', { path })
}

export function getGitConfig() {
  return invoke<GitSyncConfig>('get_git_config')
}

export function saveGitConfig(config: GitSyncConfig) {
  return invoke<void>('save_git_config', { config })
}

export function gitPush(repoPath: string) {
  return invoke<void>('git_push', { repoPath })
}

export function gitPull(repoPath: string) {
  return invoke<string>('git_pull', { repoPath })
}

export function gitSync(repoPath: string) {
  return invoke<string>('git_sync', { repoPath })
}

export function addCustomApp(name: string, path: string) {
  return invoke<void>('add_custom_app', { name, path })
}

export function setCustomPath(appId: string, customPath: string | null) {
  return invoke<void>('set_custom_path', { appId, customPath })
}

export function openPathInFileManager(path: string) {
  return invoke<void>('open_path_in_file_manager', { path })
}

export function launchApp(appId: string) {
  return invoke<void>('launch_app', { appId })
}

export function getVersion() {
  return invoke<string>('get_version')
}

export function checkUpdates() {
  return invoke<UpdateCheckResult>('check_updates')
}

export function downloadUpdate() {
  return invoke<DownloadUpdateResult>('download_update')
}

export function openDownloadedUpdate(path: string) {
  return invoke<void>('open_downloaded_update', { path })
}

export function buildSkillInventory(skillLists: SkillInventoryChunk[]): SkillRecord[] {
  const grouped = new Map<string, SkillRecord>()

  for (const { app, files } of skillLists) {
    for (const file of files) {
      const canonicalName = file.canonical_name || file.name.toLowerCase()
      const existing = grouped.get(canonicalName)

      if (existing) {
        existing.sources = Array.from(new Set([...existing.sources, app.name]))
        existing.duplicateCount += 1
        if (!existing.contentHashes.includes(file.content_hash)) {
          existing.contentHashes = [...existing.contentHashes, file.content_hash]
        }
        existing.conflicts = existing.contentHashes.length > 1

        if (file.modified > existing.modified) {
          existing.modified = file.modified
          existing.path = file.path
        }

        if (file.size > existing.size) {
          existing.size = file.size
        }

        if (!existing.description && file.description) {
          existing.description = file.description
        }

        existing.fileCount = Math.max(existing.fileCount, file.file_count)
      } else {
        grouped.set(canonicalName, {
          id: `${app.id}:${file.path}`,
          name: file.name,
          description: file.description || file.path,
          path: file.path,
          size: file.size,
          modified: file.modified,
          sources: [app.name],
          conflicts: false,
          duplicateCount: 1,
          canonicalName,
          contentHashes: [file.content_hash],
          fileCount: file.file_count,
        })
      }
    }
  }

  return Array.from(grouped.values()).sort((left, right) => {
    if (left.conflicts !== right.conflicts) {
      return left.conflicts ? -1 : 1
    }

    return left.name.localeCompare(right.name)
  })
}

export async function loadSkillInventory(apps: AppRecord[]): Promise<SkillRecord[]> {
  const readableApps = apps.filter((app) => app.isInstalled || app.isLinked)
  const skillLists = await Promise.all(
    readableApps.map(async (app) => ({
      app,
      files: await scanSkills(app.id).catch(() => [] as BackendSkillFile[]),
    })),
  )
  return buildSkillInventory(skillLists)
}
