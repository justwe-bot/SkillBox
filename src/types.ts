export interface BackendApp {
  id: string
  name: string
  path: string
  icon: string
  skill_count: number
  is_linked: boolean
  is_installed: boolean
  is_custom?: boolean
  backup_path?: string | null
  custom_path?: string | null
}

export interface AppRecord {
  id: string
  name: string
  path: string
  icon: string
  skillCount: number
  isLinked: boolean
  isInstalled: boolean
  isCustom: boolean
  backupPath?: string | null
  customPath?: string | null
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
  username: string
  branch: string
}

export interface AppPreferences {
  autoScan: boolean
  autoSync: boolean
  desktopNotifications: boolean
  theme: 'system' | 'light' | 'dark'
}
