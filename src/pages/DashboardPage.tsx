import { useEffect, useMemo, useRef, useState } from 'react'
import { flushSync } from 'react-dom'
import { Link } from 'react-router-dom'
import { open as openDialog } from '@tauri-apps/api/dialog'
import { Archive, FolderOpen, FolderPlus, RefreshCw, Scan, Search, Settings } from 'lucide-react'
import { ApplicationCard } from '../components/ApplicationCard'
import { FigmaSkillIcon } from '../components/FigmaSkillIcon'
import { GitPanel } from '../components/GitPanel'
import { Modal } from '../components/Modal'
import { SkillItem } from '../components/SkillItem'
import { ThemeToggle } from '../components/ThemeToggle'
import { LOCAL_SYNC_SOURCE, localizeBackendSuccessMessage, localizeSkillSource } from '../lib/i18n'
import { useI18n } from '../lib/i18n-context'
import { useToast } from '../components/ToastProvider'
import {
  addCustomApp,
  deleteSkill,
  getAppEnabledSkills,
  getGitConfig,
  gitPull,
  gitPush,
  gitSync,
  linkApp,
  scanGitPathSkills,
  launchApp,
  openPathInFileManager,
  renameSkill,
  saveGitConfig as persistGitConfig,
  saveAppEnabledSkills,
  saveGitPath,
  scanApps,
  scanSkills,
  setCustomPath,
  syncToGit,
  unlinkApp,
} from '../lib/tauri'
import type { AppRecord, BackendSkillFile, GitSyncConfig, ManagedSkillEntry, SkillRecord } from '../types'

type TabKey = 'applications' | 'skills'
type GitBusyAction = 'saveConfig' | 'push' | 'pull' | 'sync' | 'aggregate' | 'pickPath' | 'changePath' | null
type SkillScanProgress = {
  completed: number
  total: number
}
type LinkBusyState = {
  appId: string
  action: 'link' | 'unlink'
  appName: string
}

const SKILL_SCAN_BATCH_SIZE = 3

function waitForNextPaint() {
  return new Promise<void>((resolve) => {
    requestAnimationFrame(() => resolve())
  })
}

function mapGitPathSkillsToRecords(files: BackendSkillFile[], language: string): SkillRecord[] {
  return files
    .map((file) => ({
      id: `git:${file.path}`,
      name: file.name,
      description: file.description || file.path,
      path: file.path,
      size: file.size,
      modified: file.modified,
      sources: [LOCAL_SYNC_SOURCE],
      conflicts: false,
      duplicateCount: 1,
      canonicalName: file.canonical_name || file.name.toLowerCase(),
      contentHashes: [file.content_hash],
      fileCount: file.file_count,
    }))
    .sort((left, right) => left.name.localeCompare(right.name, language))
}

export default function DashboardPage() {
  const { language, t } = useI18n()
  const { notify } = useToast()
  const [activeTab, setActiveTab] = useState<TabKey>('applications')
  const [apps, setApps] = useState<AppRecord[]>([])
  const [skills, setSkills] = useState<SkillRecord[]>([])
  const [gitPath, setGitPath] = useState('')
  const [gitConfig, setGitConfig] = useState<GitSyncConfig>({ repoUrl: '', username: '', branch: 'main' })
  const [appLoading, setAppLoading] = useState(true)
  const [skillLoading, setSkillLoading] = useState(false)
  const [gitSkillLoading, setGitSkillLoading] = useState(false)
  const [skillScanProgress, setSkillScanProgress] = useState<SkillScanProgress>({ completed: 0, total: 0 })
  const [busyAppId, setBusyAppId] = useState<string | null>(null)
  const [linkBusyState, setLinkBusyState] = useState<LinkBusyState | null>(null)
  const [gitBusyAction, setGitBusyAction] = useState<GitBusyAction>(null)
  const [search, setSearch] = useState('')
  const [customModalOpen, setCustomModalOpen] = useState(false)
  const [editModalOpen, setEditModalOpen] = useState(false)
  const [editingApp, setEditingApp] = useState<AppRecord | null>(null)
  const [customAppName, setCustomAppName] = useState('')
  const [customAppPath, setCustomAppPath] = useState('')
  const [editPathValue, setEditPathValue] = useState('')
  const [selectedSkill, setSelectedSkill] = useState<SkillRecord | null>(null)
  const [detailModalOpen, setDetailModalOpen] = useState(false)
  const [renameModalOpen, setRenameModalOpen] = useState(false)
  const [conflictModalOpen, setConflictModalOpen] = useState(false)
  const [deleteModalOpen, setDeleteModalOpen] = useState(false)
  const [renameValue, setRenameValue] = useState('')
  const [skillBusy, setSkillBusy] = useState(false)
  const [manageSkillsOpen, setManageSkillsOpen] = useState(false)
  const [manageSkillsApp, setManageSkillsApp] = useState<AppRecord | null>(null)
  const [manageSkillsLinkMode, setManageSkillsLinkMode] = useState<'legacy' | 'managed' | null>(null)
  const [manageSkillsEntries, setManageSkillsEntries] = useState<ManagedSkillEntry[]>([])
  const [manageSkillsSearch, setManageSkillsSearch] = useState('')
  const [manageSkillsLoading, setManageSkillsLoading] = useState(false)
  const [manageSkillsSaving, setManageSkillsSaving] = useState(false)
  const [pendingGitPath, setPendingGitPath] = useState('')
  const [confirmGitPathOpen, setConfirmGitPathOpen] = useState(false)
  const gitBusy = gitBusyAction !== null
  const scanSessionRef = useRef(0)
  const scannedSkillListsRef = useRef<Map<string, { app: AppRecord; files: BackendSkillFile[] }>>(new Map())

  function beginScanSession() {
    const sessionId = scanSessionRef.current + 1
    scanSessionRef.current = sessionId
    return sessionId
  }

  function isCurrentSession(sessionId: number) {
    return scanSessionRef.current === sessionId
  }

  async function scanGitPathInBackground(path: string, sessionId: number) {
    if (!path) {
      if (isCurrentSession(sessionId)) {
        setSkills([])
        setGitSkillLoading(false)
      }
      return
    }

    setGitSkillLoading(true)
    const gitSkills = await scanGitPathSkills(path).catch(() => [])

    if (!isCurrentSession(sessionId)) {
      return
    }

    setSkills(mapGitPathSkillsToRecords(gitSkills, language))
    setGitSkillLoading(false)
  }

  async function scanSkillsInBackground(nextApps: AppRecord[], sessionId: number, showToast: boolean) {
    const readableApps = nextApps.filter((app) => app.isInstalled || app.isLinked)

    scannedSkillListsRef.current = new Map()
    setApps(nextApps.map((app) => ({ ...app, skillCount: 0 })))
    setSkillScanProgress({ completed: 0, total: readableApps.length })

    if (!readableApps.length) {
      setSkillLoading(false)

      if (showToast) {
        notify(t('dashboard.notifications.scanComplete', { count: nextApps.filter((app) => app.isInstalled).length }), 'success')
      }
      return
    }

    setSkillLoading(true)

    for (let startIndex = 0; startIndex < readableApps.length; startIndex += SKILL_SCAN_BATCH_SIZE) {
      if (!isCurrentSession(sessionId)) {
        return
      }

      const batch = readableApps.slice(startIndex, startIndex + SKILL_SCAN_BATCH_SIZE)
      const results = await Promise.all(
        batch.map(async (app) => ({
          app,
          files: await scanSkills(app.id).catch(() => [] as BackendSkillFile[]),
        })),
      )

      if (!isCurrentSession(sessionId)) {
        return
      }

      for (const result of results) {
        scannedSkillListsRef.current.set(result.app.id, result)
      }

      setApps((previousApps) =>
        previousApps.map((app) => {
          const result = scannedSkillListsRef.current.get(app.id)
          return result ? { ...app, skillCount: result.files.length } : app
        }),
      )
      setSkillScanProgress({
        completed: Math.min(startIndex + batch.length, readableApps.length),
        total: readableApps.length,
      })

      if (startIndex + SKILL_SCAN_BATCH_SIZE < readableApps.length) {
        await waitForNextPaint()
      }
    }

    if (!isCurrentSession(sessionId)) {
      return
    }

    setSkillLoading(false)

    if (showToast) {
      notify(t('dashboard.notifications.scanComplete', { count: nextApps.filter((app) => app.isInstalled).length }), 'success')
    }
  }

  async function refreshData(showToast = false) {
    const sessionId = beginScanSession()
    setAppLoading(true)
    setSkillLoading(false)
    setSkillScanProgress({ completed: 0, total: 0 })

    try {
      const [appState, configState] = await Promise.all([scanApps(), getGitConfig()])
      if (!isCurrentSession(sessionId)) {
        return
      }

      setApps(appState.apps)
      setGitPath(appState.gitPath)
      setGitConfig(configState)
      setGitSkillLoading(Boolean(appState.gitPath))
      if (!appState.gitPath) {
        setSkills([])
      }
      setAppLoading(false)

      await waitForNextPaint()
      if (!isCurrentSession(sessionId)) {
        return
      }

      void scanGitPathInBackground(appState.gitPath, sessionId)
      void scanSkillsInBackground(appState.apps, sessionId, showToast)
    } catch (error) {
      notify(t('dashboard.notifications.loadFailed', { error: String(error) }), 'error')

      if (isCurrentSession(sessionId)) {
        setAppLoading(false)
        setSkillLoading(false)
        setGitSkillLoading(false)
        setBusyAppId(null)
        setLinkBusyState(null)
      }
    }
  }

  useEffect(() => {
    void refreshData()

    return () => {
      scanSessionRef.current += 1
    }
  }, [language, notify, t])

  const filteredSkills = useMemo(() => {
    const normalizedSkills = [...skills].sort((left, right) => left.name.localeCompare(right.name, language))

    if (!search.trim()) {
      return normalizedSkills
    }

    const query = search.trim().toLowerCase()
    return normalizedSkills.filter((skill) => {
      return (
        skill.name.toLowerCase().includes(query) ||
        skill.description.toLowerCase().includes(query) ||
        skill.sources
          .map((source) => localizeSkillSource(source, language))
          .some((source) => source.toLowerCase().includes(query))
      )
    })
  }, [language, search, skills])

  const filteredManageSkills = useMemo(() => {
    const normalizedSkills = [...manageSkillsEntries].sort((left, right) => left.name.localeCompare(right.name, language))

    if (!manageSkillsSearch.trim()) {
      return normalizedSkills
    }

    const query = manageSkillsSearch.trim().toLowerCase()
    return normalizedSkills.filter((skill) => {
      return (
        skill.name.toLowerCase().includes(query) ||
        skill.description.toLowerCase().includes(query) ||
        skill.entryName.toLowerCase().includes(query)
      )
    })
  }, [language, manageSkillsEntries, manageSkillsSearch])

  const sortedApps = useMemo(() => {
    return apps
      .map((app, index) => ({ app, index }))
      .sort((left, right) => {
        if (left.app.isInstalled !== right.app.isInstalled) {
          return left.app.isInstalled ? -1 : 1
        }

        if (left.app.isLinked !== right.app.isLinked) {
          return left.app.isLinked ? -1 : 1
        }

        return left.index - right.index
      })
      .map(({ app }) => app)
  }, [apps])

  const stats = useMemo(() => {
    return {
      appCount: apps.filter((app) => app.isInstalled).length,
      skillCount: skills.length,
      linkedCount: apps.filter((app) => app.isLinked).length,
      conflictCount: skills.filter((skill) => skill.conflicts).length,
    }
  }, [apps, skills])

  const heroText = useMemo(() => {
    if (appLoading) {
      return t('dashboard.hero.loadingApps')
    }

    if (skillLoading) {
      return t('dashboard.hero.scanningSkills', {
        completed: skillScanProgress.completed,
        total: skillScanProgress.total,
      })
    }

    if (gitSkillLoading) {
      return t('dashboard.hero.updatingLocalStats')
    }

    return t('dashboard.hero.idle')
  }, [appLoading, gitSkillLoading, skillLoading, skillScanProgress.completed, skillScanProgress.total, t])

  const refreshButtonLabel = appLoading ? t('dashboard.refresh.loadingApps') : skillLoading ? t('dashboard.refresh.scanningSkills') : t('dashboard.refresh.default')
  const activeOperationLabel = useMemo(() => {
    if (linkBusyState) {
      return linkBusyState.action === 'link'
        ? t('dashboard.activeOperation.linking', { name: linkBusyState.appName })
        : t('dashboard.activeOperation.unlinking', { name: linkBusyState.appName })
    }

    if (gitBusyAction === 'aggregate') {
      return t('dashboard.activeOperation.aggregate')
    }

    if (gitBusyAction === 'push') {
      return t('dashboard.activeOperation.push')
    }

    if (gitBusyAction === 'pull') {
      return t('dashboard.activeOperation.pull')
    }

    if (gitBusyAction === 'sync') {
      return t('dashboard.activeOperation.sync')
    }

    if (gitBusyAction === 'saveConfig') {
      return t('dashboard.activeOperation.saveConfig')
    }

    if (gitBusyAction === 'pickPath') {
      return t('dashboard.activeOperation.pickPath')
    }

    if (gitBusyAction === 'changePath') {
      return t('dashboard.activeOperation.changePath')
    }

    return null
  }, [gitBusyAction, linkBusyState, t])

  async function handleToggleLink(app: AppRecord) {
    if (!app.isLinked && !gitPath) {
      notify(t('dashboard.notifications.needGitPathBeforeLink'), 'error')
      return
    }

    // Optimistic update — flip isLinked immediately so the UI responds at once
    const nextLinked = !app.isLinked
    setApps((prev) =>
      prev.map((a) => (a.id === app.id ? { ...a, isLinked: nextLinked } : a)),
    )
    setBusyAppId(app.id)
    setLinkBusyState({
      appId: app.id,
      action: app.isLinked ? 'unlink' : 'link',
      appName: app.name,
    })

    try {
      if (app.isLinked) {
        await unlinkApp(app.id)
        notify(t('dashboard.notifications.linkRemoved', { name: app.name }), 'success')
      } else {
        await linkApp(app.id, gitPath)
        notify(t('dashboard.notifications.linkCreated', { name: app.name }), 'success')
      }
    } catch (error) {
      // Roll back the optimistic update on failure
      setApps((prev) =>
        prev.map((a) => (a.id === app.id ? { ...a, isLinked: app.isLinked } : a)),
      )
      notify(t('dashboard.notifications.operationFailed', { error: String(error) }), 'error')
      setBusyAppId(null)
      setLinkBusyState(null)
      return
    }

    setBusyAppId(null)
    setLinkBusyState(null)

    // Refresh in the background using staged batches so the page stays responsive.
    void refreshData()
  }

  async function handleOpenAppFolder(app: AppRecord) {
    try {
      await openPathInFileManager(app.path)
    } catch (error) {
      notify(t('dashboard.notifications.openFolderFailed', { error: String(error) }), 'error')
    }
  }

  async function handleLaunchApp(app: AppRecord) {
    try {
      await launchApp(app.id)
    } catch (error) {
      notify(t('dashboard.notifications.launchFailed', { error: String(error) }), 'error')
    }
  }

  async function handleOpenGitFolder() {
    if (!gitPath) {
      return
    }

    try {
      await openPathInFileManager(gitPath)
    } catch (error) {
      notify(t('dashboard.notifications.openSyncFolderFailed', { error: String(error) }), 'error')
    }
  }

  async function handleAggregateSkills() {
    if (gitBusy || !gitPath) {
      notify(t('dashboard.notifications.chooseSyncDirFirst'), 'error')
      return
    }

    flushSync(() => setGitBusyAction('aggregate'))
    await waitForNextPaint()
    try {
      await syncToGit(gitPath)
      await refreshData()
      notify(t('dashboard.notifications.aggregateSuccess'), 'success')
    } catch (error) {
      notify(t('dashboard.notifications.aggregateFailed', { error: String(error) }), 'error')
    } finally {
      setGitBusyAction(null)
    }
  }

  async function handlePickGitFolder() {
    if (gitBusy) {
      return
    }

    flushSync(() => setGitBusyAction('pickPath'))
    await waitForNextPaint()
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: t('dashboard.gitDialogTitle'),
    })

    if (typeof selected === 'string' && selected) {
      setPendingGitPath(selected)
      setConfirmGitPathOpen(true)
    }

    setGitBusyAction(null)
  }

  async function handleConfirmGitFolderChange() {
    if (gitBusy || !pendingGitPath) {
      return
    }

    flushSync(() => setGitBusyAction('changePath'))
    await waitForNextPaint()
    try {
      await saveGitPath(pendingGitPath)
      await syncToGit(pendingGitPath)
      setGitPath(pendingGitPath)
      setConfirmGitPathOpen(false)
      setPendingGitPath('')
      await refreshData()
      notify(t('dashboard.notifications.syncPathUpdated'), 'success')
    } catch (error) {
      notify(t('dashboard.notifications.updateSyncPathFailed', { error: String(error) }), 'error')
    } finally {
      setGitBusyAction(null)
    }
  }

  async function handleSync() {
    if (gitBusy || !gitPath) {
      notify(t('dashboard.notifications.chooseSyncDirFirst'), 'error')
      return
    }

    flushSync(() => setGitBusyAction('sync'))
    await waitForNextPaint()
    try {
      const message = await gitSync(gitPath)
      await refreshData()
      notify(localizeBackendSuccessMessage(message, language), 'success')
    } catch (error) {
      notify(t('dashboard.notifications.syncFailed', { error: String(error) }), 'error')
    } finally {
      setGitBusyAction(null)
    }
  }

  async function handlePush() {
    if (gitBusy || !gitPath) {
      notify(t('dashboard.notifications.chooseSyncDirFirst'), 'error')
      return
    }

    flushSync(() => setGitBusyAction('push'))
    await waitForNextPaint()
    try {
      await gitPush(gitPath)
      await refreshData()
      notify(t('dashboard.notifications.pushSuccess'), 'success')
    } catch (error) {
      notify(t('dashboard.notifications.pushFailed', { error: String(error) }), 'error')
    } finally {
      setGitBusyAction(null)
    }
  }

  async function handlePull() {
    if (gitBusy || !gitPath) {
      notify(t('dashboard.notifications.chooseSyncDirFirst'), 'error')
      return
    }

    flushSync(() => setGitBusyAction('pull'))
    await waitForNextPaint()
    try {
      const message = await gitPull(gitPath)
      await refreshData()
      notify(localizeBackendSuccessMessage(message, language), 'success')
    } catch (error) {
      notify(t('dashboard.notifications.pullFailed', { error: String(error) }), 'error')
    } finally {
      setGitBusyAction(null)
    }
  }

  async function handleSaveGitConfig(config: GitSyncConfig) {
    if (gitBusy) {
      return
    }

    flushSync(() => setGitBusyAction('saveConfig'))
    await waitForNextPaint()
    try {
      await persistGitConfig(config)
      setGitConfig(config)
      notify(t('dashboard.notifications.gitConfigSaved'), 'success')
    } catch (error) {
      notify(t('dashboard.notifications.saveConfigFailed', { error: String(error) }), 'error')
    } finally {
      setGitBusyAction(null)
    }
  }

  async function chooseCustomPath(setter: (path: string) => void) {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: t('dashboard.pathDialogTitle'),
    })

    if (typeof selected === 'string' && selected) {
      setter(selected)
    }
  }

  async function handleAddCustomApp() {
    if (!customAppName.trim() || !customAppPath.trim()) {
      notify(t('dashboard.notifications.fillCustomAppFields'), 'error')
      return
    }

    try {
      await addCustomApp(customAppName.trim(), customAppPath.trim())
      setCustomAppName('')
      setCustomAppPath('')
      setCustomModalOpen(false)
      await refreshData()
      notify(t('dashboard.notifications.customAppAdded'), 'success')
    } catch (error) {
      notify(t('dashboard.notifications.addFailed', { error: String(error) }), 'error')
    }
  }

  function openEditPath(app: AppRecord) {
    setEditingApp(app)
    setEditPathValue(app.customPath ?? app.path)
    setEditModalOpen(true)
  }

  async function handleSavePath() {
    if (!editingApp) {
      return
    }

    try {
      const customPath = editPathValue.trim() ? editPathValue.trim() : null
      await setCustomPath(editingApp.id, customPath)
      setEditModalOpen(false)
      setEditingApp(null)
      await refreshData()
      notify(t('dashboard.notifications.pathUpdated'), 'success')
    } catch (error) {
      notify(t('dashboard.notifications.updatePathFailed', { error: String(error) }), 'error')
    }
  }

  function handleViewSkill(skill: SkillRecord) {
    setSelectedSkill(skill)
    setDetailModalOpen(true)
  }

  function handleStartRenameSkill(skill: SkillRecord) {
    setSelectedSkill(skill)
    setRenameValue(skill.name)
    setRenameModalOpen(true)
  }

  function handleResolveConflict(skill: SkillRecord) {
    setSelectedSkill(skill)
    setConflictModalOpen(true)
  }

  function handleAskDeleteSkill(skill: SkillRecord) {
    setSelectedSkill(skill)
    setDeleteModalOpen(true)
  }

  function closeManageSkillsModal(force = false) {
    if (manageSkillsSaving && !force) {
      return
    }

    setManageSkillsOpen(false)
    setManageSkillsApp(null)
    setManageSkillsLinkMode(null)
    setManageSkillsEntries([])
    setManageSkillsSearch('')
    setManageSkillsLoading(false)
  }

  async function handleOpenManageSkills(app: AppRecord) {
    if (!gitPath) {
      notify(t('dashboard.notifications.needSyncDirBeforeManage'), 'error')
      return
    }

    setManageSkillsApp(app)
    setManageSkillsOpen(true)
    setManageSkillsLoading(true)
    setManageSkillsSearch('')

    try {
      const result = await getAppEnabledSkills(app.id, gitPath)
      setManageSkillsLinkMode(result.linkMode)
      setManageSkillsEntries(result.skills)

      if (result.linkMode === 'legacy') {
        notify(t('dashboard.notifications.legacyLinkDetected'), 'info')
      }
    } catch (error) {
      closeManageSkillsModal()
      notify(t('dashboard.notifications.loadManageSkillsFailed', { error: String(error) }), 'error')
    } finally {
      setManageSkillsLoading(false)
    }
  }

  function handleToggleManagedSkill(entryName: string) {
    setManageSkillsEntries((current) =>
      current.map((skill) => (skill.entryName === entryName ? { ...skill, enabled: !skill.enabled } : skill)),
    )
  }

  function handleSetAllManagedSkills(enabled: boolean) {
    setManageSkillsEntries((current) => current.map((skill) => ({ ...skill, enabled })))
  }

  async function handleSaveManagedSkills() {
    if (!manageSkillsApp || !gitPath) {
      return
    }

    setManageSkillsSaving(true)
    try {
      const enabledEntries = manageSkillsEntries.filter((skill) => skill.enabled).map((skill) => skill.entryName)
      await saveAppEnabledSkills(manageSkillsApp.id, gitPath, enabledEntries)
      closeManageSkillsModal(true)
      await refreshData()
      notify(t('dashboard.notifications.manageSkillsUpdated', { name: manageSkillsApp.name }), 'success')
    } catch (error) {
      notify(t('dashboard.notifications.saveManageSkillsFailed', { error: String(error) }), 'error')
    } finally {
      setManageSkillsSaving(false)
    }
  }

  async function handleConfirmRenameSkill() {
    if (!selectedSkill || !renameValue.trim()) {
      notify(t('dashboard.notifications.enterNewSkillName'), 'error')
      return
    }

    setSkillBusy(true)
    try {
      await renameSkill(selectedSkill.path, renameValue.trim())
      setRenameModalOpen(false)
      setConflictModalOpen(false)
      setSelectedSkill(null)
      await refreshData()
      notify(t('dashboard.notifications.skillRenamed'), 'success')
    } catch (error) {
      notify(t('dashboard.notifications.renameFailed', { error: String(error) }), 'error')
    } finally {
      setSkillBusy(false)
    }
  }

  async function handleConfirmDeleteSkill() {
    if (!selectedSkill) {
      return
    }

    setSkillBusy(true)
    try {
      await deleteSkill(selectedSkill.path)
      setDeleteModalOpen(false)
      setConflictModalOpen(false)
      setSelectedSkill(null)
      await refreshData()
      notify(t('dashboard.notifications.skillDeleted'), 'success')
    } catch (error) {
      notify(t('dashboard.notifications.deleteFailed', { error: String(error) }), 'error')
    } finally {
      setSkillBusy(false)
    }
  }

  return (
    <div className="page-shell">
      <header className="hero hero--dashboard">
        <div className="hero__brand">
          <FigmaSkillIcon size={48} />
          <div>
            <h1>{t('dashboard.title')}</h1>
            <p className="hero__text">{heroText}</p>
          </div>
        </div>
        <div className="hero__actions">
          <button className="button button--primary button--hero" type="button" onClick={() => void refreshData(true)} disabled={appLoading}>
            <Scan size={18} />
            {refreshButtonLabel}
          </button>
          <ThemeToggle />
          <Link className="button button--square" to="/settings" aria-label={t('dashboard.openSettings')}>
            <Settings size={22} />
          </Link>
        </div>
      </header>

      <section className="stats-grid">
        <article className="surface stat-card">
          <span className="stat-card__label">{t('dashboard.stats.detectedApps')}</span>
          <strong>{stats.appCount}</strong>
        </article>
        <article className="surface stat-card">
          <span className="stat-card__label">{t('dashboard.stats.skillFiles')}</span>
          <strong>{gitSkillLoading ? t('dashboard.stats.scanning') : stats.skillCount}</strong>
        </article>
        <article className="surface stat-card">
          <span className="stat-card__label">{t('dashboard.stats.linkedApps')}</span>
          <strong>{stats.linkedCount}</strong>
        </article>
        <article className="surface stat-card">
          <span className="stat-card__label">{t('dashboard.stats.conflicts')}</span>
          <strong className={stats.conflictCount ? 'danger' : ''}>{stats.conflictCount}</strong>
        </article>
      </section>

      {activeOperationLabel ? (
        <section className="operation-strip">
          <div className="surface operation-banner" aria-live="polite" aria-busy="true">
            <RefreshCw size={16} className="spin" />
            <span>{activeOperationLabel}</span>
          </div>
        </section>
      ) : null}

      <main className="dashboard-grid">
        <section className="main-column">
          {activeTab === 'applications' ? (
            <section className="stack">
              <div className="section-toolbar section-toolbar--applications">
                <div className="tabs tabs--embedded">
                  <button className="tab tab--active" type="button" onClick={() => setActiveTab('applications')}>
                    {t('dashboard.tabs.apps')}
                  </button>
                  <button className="tab" type="button" onClick={() => setActiveTab('skills')}>
                    {t('dashboard.tabs.skills')}
                    {stats.conflictCount ? <span className="tab__count tab__count--danger">{stats.conflictCount}</span> : null}
                  </button>
                </div>
                <button className="button button--ghost" type="button" onClick={() => setCustomModalOpen(true)}>
                  <FolderPlus size={16} />
                  {t('dashboard.addCustomApp')}
                </button>
              </div>

              {sortedApps.length === 0 ? (
                <div className="surface empty-state">
                  <p>{appLoading ? t('dashboard.empty.appsLoading') : t('dashboard.empty.apps')}</p>
                </div>
              ) : (
                sortedApps.map((app) => (
                  <ApplicationCard
                    key={app.id}
                    app={app}
                    totalSkillCount={skills.length}
                    busy={busyAppId === app.id}
                    busyLabel={
                      linkBusyState?.appId === app.id
                        ? linkBusyState.action === 'link'
                          ? t('dashboard.card.linking')
                          : t('dashboard.card.unlinking')
                        : null
                    }
                    onManageSkills={handleOpenManageSkills}
                    onToggleLink={handleToggleLink}
                    onOpenFolder={handleOpenAppFolder}
                    onLaunchApp={handleLaunchApp}
                    onEditPath={openEditPath}
                  />
                ))
              )}
            </section>
          ) : (
            <section className="stack">
              <div className="section-toolbar section-toolbar--align-start section-toolbar--applications">
                <div className="tabs tabs--embedded">
                  <button className="tab" type="button" onClick={() => setActiveTab('applications')}>
                    {t('dashboard.tabs.apps')}
                  </button>
                  <button className="tab tab--active" type="button">
                    {t('dashboard.tabs.skills')}
                    {stats.conflictCount ? <span className="tab__count">{stats.conflictCount}</span> : null}
                  </button>
                </div>
                <label className="search-box">
                  <Search size={16} />
                  <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder={t('dashboard.dialog.searchSkills')} />
                </label>
              </div>

              {stats.conflictCount ? (
                <div className="surface conflict-banner">
                  <div className="conflict-banner__body">
                    <strong>{t('dashboard.conflicts.summary', { count: stats.conflictCount })}</strong>
                    <p>{t('dashboard.conflicts.detail')}</p>
                  </div>
                </div>
              ) : null}

              {filteredSkills.length === 0 ? (
                <div className="surface empty-state">
                  <p>{search ? t('dashboard.empty.noSkillMatch') : skillLoading ? t('dashboard.empty.skillsLoading') : t('dashboard.empty.skills')}</p>
                </div>
              ) : (
                filteredSkills.map((skill) => (
                  <SkillItem
                    key={skill.id}
                    skill={skill}
                    onView={handleViewSkill}
                    onRename={handleStartRenameSkill}
                    onDelete={handleAskDeleteSkill}
                    onResolveConflict={handleResolveConflict}
                  />
                ))
              )}
            </section>
          )}
        </section>

        <aside className="side-column">
          <GitPanel
            gitPath={gitPath}
            gitConfig={gitConfig}
            busyAction={gitBusyAction}
            onSaveConfig={handleSaveGitConfig}
            onPush={handlePush}
            onPull={handlePull}
            onSync={handleSync}
          />
          {gitPath ? (
            <>
              <div className="surface side-column__path-card">
                <div className="side-column__path-actions">
                  <button
                    className="button button--icon-ghost side-column__path-action"
                    type="button"
                    onClick={() => void handleOpenGitFolder()}
                    aria-label={t('dashboard.side.openSyncDirectory')}
                    title={t('dashboard.side.openFolder')}
                  >
                    <FolderOpen size={18} />
                  </button>
                </div>
                <span className="side-column__path-label">{t('common.localSyncDirectory')}</span>
                <strong className="side-column__path-value">{gitPath}</strong>
              </div>
              <div className="side-column__path-buttons">
                <button
                  className="button button--ghost side-column__path-primary"
                  type="button"
                  onClick={() => void handlePickGitFolder()}
                  disabled={gitBusy}
                >
                  {gitBusyAction === 'pickPath' || gitBusyAction === 'changePath' ? <RefreshCw size={16} className="spin" /> : null}
                  {gitBusyAction === 'pickPath' || gitBusyAction === 'changePath' ? t('dashboard.side.changePathLoading') : t('dashboard.side.changePath')}
                </button>
                <button
                  className="button button--primary side-column__path-primary"
                  type="button"
                  onClick={() => void handleAggregateSkills()}
                  disabled={gitBusy}
                >
                  {gitBusyAction === 'aggregate' ? <RefreshCw size={17} className="spin" /> : <Archive size={17} />}
                  {gitBusyAction === 'aggregate' ? t('dashboard.side.aggregateLoading') : t('dashboard.side.aggregate')}
                </button>
              </div>
            </>
          ) : null}
          {!gitPath ? (
            <button
              className="button button--ghost button--full side-column__picker"
              type="button"
              onClick={() => void handlePickGitFolder()}
              disabled={gitBusy}
            >
              {gitBusyAction === 'pickPath' ? <RefreshCw size={16} className="spin" /> : null}
              {gitBusyAction === 'pickPath' ? t('dashboard.side.pickDirectoryLoading') : t('dashboard.side.pickDirectory')}
            </button>
          ) : null}
        </aside>
      </main>

      <Modal open={customModalOpen} title={t('dashboard.dialog.addCustomApp')} onClose={() => setCustomModalOpen(false)}>
        <div className="modal__body">
          <div className="field-group">
            <label htmlFor="custom-app-name">{t('common.appName')}</label>
            <input
              id="custom-app-name"
              value={customAppName}
              onChange={(event) => setCustomAppName(event.target.value)}
              placeholder={t('dashboard.dialog.customAppNamePlaceholder')}
            />
          </div>
          <div className="field-group">
            <label htmlFor="custom-app-path">{t('common.skillDirectory')}</label>
            <div className="inline-field">
              <input
                id="custom-app-path"
                value={customAppPath}
                onChange={(event) => setCustomAppPath(event.target.value)}
                placeholder={t('dashboard.dialog.customAppPathPlaceholder')}
              />
              <button className="button button--ghost" type="button" onClick={() => void chooseCustomPath(setCustomAppPath)}>
                {t('common.browse')}
              </button>
            </div>
          </div>
        </div>
        <footer className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => setCustomModalOpen(false)}>
            {t('common.cancel')}
          </button>
          <button className="button button--primary" type="button" onClick={() => void handleAddCustomApp()}>
            {t('dashboard.dialog.add')}
          </button>
        </footer>
      </Modal>

      <Modal open={editModalOpen} title={editingApp ? t('dashboard.dialog.editPath', { name: editingApp.name }) : t('dashboard.dialog.editPathFallback')} onClose={() => setEditModalOpen(false)}>
        <div className="modal__body">
          <div className="field-group">
            <label htmlFor="edit-app-path">{t('common.skillDirectory')}</label>
            <div className="inline-field">
              <input
                id="edit-app-path"
                value={editPathValue}
                onChange={(event) => setEditPathValue(event.target.value)}
                placeholder={t('dashboard.dialog.emptyPathPlaceholder')}
              />
              <button className="button button--ghost" type="button" onClick={() => void chooseCustomPath(setEditPathValue)}>
                {t('common.browse')}
              </button>
            </div>
          </div>
        </div>
        <footer className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => setEditPathValue('')}>
            {t('dashboard.dialog.restoreDefault')}
          </button>
          <button className="button button--primary" type="button" onClick={() => void handleSavePath()}>
            {t('common.save')}
          </button>
        </footer>
      </Modal>

      <Modal open={detailModalOpen} title={selectedSkill?.name ?? t('dashboard.dialog.skillDetails')} onClose={() => setDetailModalOpen(false)}>
        {selectedSkill ? (
          <>
            <div className="modal__body modal__body--stack">
              <div className="detail-grid">
                <div className="detail-field">
                  <span>{t('common.description')}</span>
                  <strong>{selectedSkill.description || t('dashboard.dialog.noDescription')}</strong>
                </div>
                <div className="detail-field">
                  <span>{t('common.source')}</span>
                  <strong>{selectedSkill.sources.map((source) => localizeSkillSource(source, language)).join(', ')}</strong>
                </div>
                <div className="detail-field">
                  <span>{t('common.path')}</span>
                  <strong className="detail-field__path">{selectedSkill.path}</strong>
                </div>
                <div className="detail-field">
                  <span>{t('common.size')}</span>
                  <strong>{(selectedSkill.size / 1024).toFixed(1)} KB</strong>
                </div>
                <div className="detail-field">
                  <span>{t('common.fileCount')}</span>
                  <strong>{selectedSkill.fileCount}</strong>
                </div>
                <div className="detail-field">
                  <span>{t('common.modifiedAt')}</span>
                  <strong>{selectedSkill.modified}</strong>
                </div>
              </div>
            </div>
            <footer className="modal__footer">
              <button className="button button--ghost" type="button" onClick={() => setDetailModalOpen(false)}>
                {t('common.close')}
              </button>
              <button className="button button--primary" type="button" onClick={() => {
                setDetailModalOpen(false)
                handleStartRenameSkill(selectedSkill)
              }}>
                {t('dashboard.dialog.renameAction')}
              </button>
            </footer>
          </>
        ) : null}
      </Modal>

      <Modal open={renameModalOpen} title={selectedSkill ? t('dashboard.dialog.renameSkill', { name: selectedSkill.name }) : t('dashboard.dialog.renameSkillFallback')} onClose={() => setRenameModalOpen(false)}>
        <div className="modal__body">
          <div className="field-group">
            <label htmlFor="rename-skill-name">{t('common.skillName')}</label>
            <input
              id="rename-skill-name"
              value={renameValue}
              onChange={(event) => setRenameValue(event.target.value)}
              placeholder={t('dashboard.dialog.renameSkillPlaceholder')}
            />
          </div>
        </div>
        <footer className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => setRenameModalOpen(false)}>
            {t('common.cancel')}
          </button>
          <button className="button button--primary" type="button" onClick={() => void handleConfirmRenameSkill()} disabled={skillBusy}>
            {t('common.save')}
          </button>
        </footer>
      </Modal>

      <Modal
        open={manageSkillsOpen}
        title={manageSkillsApp ? t('dashboard.dialog.manageSkills', { name: manageSkillsApp.name }) : t('dashboard.dialog.manageSkillsFallback')}
        onClose={closeManageSkillsModal}
      >
        <div className="modal__body modal__body--stack">
          <p className="modal__copy">
            {manageSkillsLinkMode === 'legacy'
              ? t('dashboard.dialog.manageSkillsLegacyHint')
              : t('dashboard.dialog.manageSkillsHint')}
          </p>

          <div className="manage-skills__toolbar">
            <label className="search-box manage-skills__search">
              <Search size={16} />
              <input value={manageSkillsSearch} onChange={(event) => setManageSkillsSearch(event.target.value)} placeholder={t('dashboard.dialog.searchSkills')} />
            </label>
            <div className="manage-skills__actions">
              <button className="button button--ghost button--compact" type="button" onClick={() => handleSetAllManagedSkills(true)} disabled={manageSkillsLoading}>
                {t('dashboard.dialog.selectAll')}
              </button>
              <button className="button button--ghost button--compact" type="button" onClick={() => handleSetAllManagedSkills(false)} disabled={manageSkillsLoading}>
                {t('dashboard.dialog.clearAll')}
              </button>
            </div>
          </div>

          <div className="manage-skills__summary">
            {t('dashboard.dialog.enabledSummary', {
              enabled: manageSkillsEntries.filter((skill) => skill.enabled).length,
              total: manageSkillsEntries.length,
            })}
          </div>

          {manageSkillsLoading ? (
            <div className="surface empty-state">
              <p>{t('dashboard.dialog.manageSkillsLoading')}</p>
            </div>
          ) : filteredManageSkills.length === 0 ? (
            <div className="surface empty-state">
              <p>{manageSkillsSearch ? t('dashboard.empty.noSkillMatch') : t('dashboard.dialog.manageSkillsEmpty')}</p>
            </div>
          ) : (
            <div className="manage-skills__list">
              {filteredManageSkills.map((skill) => (
                <article key={skill.entryName} className={`surface manage-skills__item ${skill.enabled ? 'manage-skills__item--enabled' : ''}`}>
                  <label className="manage-skills__item-main">
                    <input
                      className="manage-skills__checkbox"
                      type="checkbox"
                      checked={skill.enabled}
                      onChange={() => handleToggleManagedSkill(skill.entryName)}
                    />
                    <div className="manage-skills__item-copy">
                      <strong>{skill.name}</strong>
                      <span>{skill.description || skill.entryName}</span>
                      <small>{skill.entryName}</small>
                    </div>
                  </label>
                  <span className="manage-skills__status">
                    {skill.enabled ? t('dashboard.dialog.enabled') : t('dashboard.dialog.disabled')}
                  </span>
                </article>
              ))}
            </div>
          )}
        </div>
        <footer className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => closeManageSkillsModal()} disabled={manageSkillsSaving}>
            {t('common.cancel')}
          </button>
          <button className="button button--primary" type="button" onClick={() => void handleSaveManagedSkills()} disabled={manageSkillsLoading || manageSkillsSaving}>
            {manageSkillsSaving ? t('common.saving') : t('common.save')}
          </button>
        </footer>
      </Modal>

      <Modal open={deleteModalOpen} title={selectedSkill ? t('dashboard.dialog.deleteSkill', { name: selectedSkill.name }) : t('dashboard.dialog.deleteSkillFallback')} onClose={() => setDeleteModalOpen(false)}>
        <div className="modal__body modal__body--stack">
          <p className="modal__copy">{t('dashboard.dialog.deleteWarning')}</p>
          {selectedSkill ? (
            <div className="detail-field">
              <span>{t('common.targetPath')}</span>
              <strong className="detail-field__path">{selectedSkill.path}</strong>
            </div>
          ) : null}
        </div>
        <footer className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => setDeleteModalOpen(false)}>
            {t('common.cancel')}
          </button>
          <button className="button button--primary button--danger" type="button" onClick={() => void handleConfirmDeleteSkill()} disabled={skillBusy}>
            {t('common.delete')}
          </button>
        </footer>
      </Modal>

      <Modal open={conflictModalOpen} title={selectedSkill ? t('dashboard.dialog.conflictTitle', { name: selectedSkill.name }) : t('dashboard.dialog.conflictFallback')} onClose={() => setConflictModalOpen(false)}>
        <div className="modal__body modal__body--stack">
          {selectedSkill ? (
            <>
              <p className="modal__copy">
                {t('dashboard.dialog.conflictMessage', {
                  count: selectedSkill.duplicateCount,
                  sources: selectedSkill.sources.map((source) => localizeSkillSource(source, language)).join(language === 'en-US' ? ', ' : '、'),
                })}
              </p>
              <div className="detail-grid">
                <div className="detail-field">
                  <span>{t('dashboard.dialog.canonicalName')}</span>
                  <strong>{selectedSkill.canonicalName}</strong>
                </div>
                <div className="detail-field">
                  <span>{t('common.path')}</span>
                  <strong className="detail-field__path">{selectedSkill.path}</strong>
                </div>
              </div>
            </>
          ) : null}
        </div>
        <footer className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => setConflictModalOpen(false)}>
            {t('dashboard.dialog.later')}
          </button>
          <button className="button button--ghost" type="button" onClick={() => {
            setConflictModalOpen(false)
            if (selectedSkill) {
              handleAskDeleteSkill(selectedSkill)
            }
          }}>
            {t('dashboard.dialog.deleteCurrent')}
          </button>
          <button className="button button--primary" type="button" onClick={() => {
            setConflictModalOpen(false)
            if (selectedSkill) {
              handleStartRenameSkill(selectedSkill)
            }
          }}>
            {t('dashboard.dialog.renameCurrent')}
          </button>
        </footer>
      </Modal>

      <Modal open={confirmGitPathOpen} title={t('dashboard.dialog.confirmChangeSyncDir')} onClose={() => {
        setConfirmGitPathOpen(false)
        setPendingGitPath('')
      }}>
        <div className="modal__body modal__body--stack">
          <p className="modal__copy">{t('dashboard.dialog.confirmChangeSyncDirMessage')}</p>
          {pendingGitPath ? (
            <div className="detail-field">
              <span>{t('common.targetDirectory')}</span>
              <strong className="detail-field__path">{pendingGitPath}</strong>
            </div>
          ) : null}
        </div>
        <footer className="modal__footer">
          <button
            className="button button--ghost"
            type="button"
            onClick={() => {
              if (gitBusyAction === 'changePath') {
                return
              }
              setConfirmGitPathOpen(false)
              setPendingGitPath('')
            }}
            disabled={gitBusyAction === 'changePath'}
          >
            {t('common.cancel')}
          </button>
          <button className="button button--primary" type="button" onClick={() => void handleConfirmGitFolderChange()} disabled={gitBusyAction === 'changePath'}>
            {gitBusyAction === 'changePath' ? t('dashboard.dialog.syncing') : t('dashboard.dialog.confirmAndSync')}
          </button>
        </footer>
      </Modal>
    </div>
  )
}
