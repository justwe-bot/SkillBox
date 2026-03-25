export interface BackendApp {
  id: string
  name: string
  path: string
  icon: string
  skill_count: number
  enabled_skill_count: number
  is_linked: boolean
  is_installed: boolean
  is_custom?: boolean
  backup_path?: string | null
  custom_path?: string | null
  link_mode?: 'legacy' | 'managed' | null
}

export interface AppRecord {
  id: string
  name: string
  path: string
  icon: string
  skillCount: number
  enabledSkillCount: number
  isLinked: boolean
  isInstalled: boolean
  isCustom: boolean
  backupPath?: string | null
  customPath?: string | null
  linkMode?: 'legacy' | 'managed' | null
}

export interface BackendSkillFile {
  name: string
  path: string
  size: number
  modified: string
  description: string
  canonical_name: string
  content_hash: string
  file_count: number
}

export interface SkillRecord {
  id: string
  name: string
  description: string
  path: string
  size: number
  modified: string
  sources: string[]
  conflicts: boolean
  duplicateCount: number
  canonicalName: string
  contentHashes: string[]
  fileCount: number
}

export interface ScanAppsResponseObject {
  apps: BackendApp[]
  gitPath?: string
  git_path?: string
}

export type ScanAppsResponse = [BackendApp[], string] | ScanAppsResponseObject

export interface GitSyncConfig {
  repoUrl: string
  branch: string
}

export type AppLanguage = 'zh-CN' | 'en-US'

export interface AppPreferences {
  autoScan: boolean
  autoSync: boolean
  desktopNotifications: boolean
  theme: 'system' | 'light' | 'dark'
  language: AppLanguage
  onboardingCompleted: boolean
}

export interface UpdateCheckResult {
  currentVersion: string
  latestVersion: string | null
  updateAvailable: boolean
  releaseUrl: string
  releaseName: string | null
  publishedAt: string | null
  notes: string | null
}

export interface DownloadUpdateResult {
  version: string
  fileName: string
  filePath: string
  releaseUrl: string
}

export interface BackendManagedSkillEntry extends BackendSkillFile {
  entryName: string
  enabled: boolean
}

export interface BackendAppEnabledSkillsResponse {
  appId: string
  linkMode: 'legacy' | 'managed'
  enabledEntries: string[]
  skills: BackendManagedSkillEntry[]
}

export interface ManagedSkillEntry extends SkillRecord {
  entryName: string
  enabled: boolean
}

export interface AppEnabledSkillsState {
  appId: string
  linkMode: 'legacy' | 'managed'
  enabledEntries: string[]
  skills: ManagedSkillEntry[]
}
