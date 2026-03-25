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
import { useToast } from '../components/ToastProvider'
import {
  addCustomApp,
  deleteSkill,
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
  saveGitPath,
  scanApps,
  scanSkills,
  setCustomPath,
  syncToGit,
  unlinkApp,
} from '../lib/tauri'
import type { AppRecord, BackendSkillFile, GitSyncConfig, SkillRecord } from '../types'

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

function mapGitPathSkillsToRecords(files: BackendSkillFile[]): SkillRecord[] {
  return files
    .map((file) => ({
      id: `git:${file.path}`,
      name: file.name,
      description: file.description || file.path,
      path: file.path,
      size: file.size,
      modified: file.modified,
      sources: ['本地同步目录'],
      conflicts: false,
      duplicateCount: 1,
      canonicalName: file.canonical_name || file.name.toLowerCase(),
      contentHashes: [file.content_hash],
      fileCount: file.file_count,
    }))
    .sort((left, right) => left.name.localeCompare(right.name))
}

export default function DashboardPage() {
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

    setSkills(mapGitPathSkillsToRecords(gitSkills))
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
        notify(`扫描完成，共检测到 ${nextApps.filter((app) => app.isInstalled).length} 个应用`, 'success')
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
      notify(`扫描完成，共检测到 ${nextApps.filter((app) => app.isInstalled).length} 个应用`, 'success')
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
      notify(`加载失败: ${String(error)}`, 'error')

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
  }, [])

  const filteredSkills = useMemo(() => {
    if (!search.trim()) {
      return skills
    }

    const query = search.trim().toLowerCase()
    return skills.filter((skill) => {
      return (
        skill.name.toLowerCase().includes(query) ||
        skill.description.toLowerCase().includes(query) ||
        skill.sources.some((source) => source.toLowerCase().includes(query))
      )
    })
  }, [search, skills])

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
      return '正在检测已安装的应用...'
    }

    if (skillLoading) {
      return `技能后台扫描中，已完成 ${skillScanProgress.completed}/${skillScanProgress.total}`
    }

    if (gitSkillLoading) {
      return '正在更新本地同步目录中的技能统计...'
    }

    return '统一管理所有 AI 应用的技能'
  }, [appLoading, gitSkillLoading, skillLoading, skillScanProgress.completed, skillScanProgress.total])

  const refreshButtonLabel = appLoading ? '检测应用中...' : skillLoading ? '后台扫描中...' : '扫描应用'
  const activeOperationLabel = useMemo(() => {
    if (linkBusyState) {
      return linkBusyState.action === 'link'
        ? `正在为 ${linkBusyState.appName} 创建链接...`
        : `正在取消 ${linkBusyState.appName} 的链接...`
    }

    if (gitBusyAction === 'aggregate') {
      return '正在汇总技能到本地同步目录...'
    }

    if (gitBusyAction === 'push') {
      return '正在推送到远程仓库...'
    }

    if (gitBusyAction === 'pull') {
      return '正在从远程仓库拉取...'
    }

    if (gitBusyAction === 'sync') {
      return '正在同步本地与远程技能...'
    }

    if (gitBusyAction === 'saveConfig') {
      return '正在保存 Git 配置...'
    }

    if (gitBusyAction === 'pickPath') {
      return '正在选择本地同步目录...'
    }

    if (gitBusyAction === 'changePath') {
      return '正在切换本地同步目录...'
    }

    return null
  }, [gitBusyAction, linkBusyState])

  async function handleToggleLink(app: AppRecord) {
    if (!app.isLinked && !gitPath) {
      notify('请先配置 Git 同步目录，再执行链接。', 'error')
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
        notify(`已取消 ${app.name} 的软链接`, 'success')
      } else {
        await linkApp(app.id, gitPath)
        notify(`已创建 ${app.name} 的软链接`, 'success')
      }
    } catch (error) {
      // Roll back the optimistic update on failure
      setApps((prev) =>
        prev.map((a) => (a.id === app.id ? { ...a, isLinked: app.isLinked } : a)),
      )
      notify(`操作失败: ${String(error)}`, 'error')
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
      notify(`打开文件夹失败: ${String(error)}`, 'error')
    }
  }

  async function handleLaunchApp(app: AppRecord) {
    try {
      await launchApp(app.id)
    } catch (error) {
      notify(`运行软件失败: ${String(error)}`, 'error')
    }
  }

  async function handleOpenGitFolder() {
    if (!gitPath) {
      return
    }

    try {
      await openPathInFileManager(gitPath)
    } catch (error) {
      notify(`打开本地同步目录失败: ${String(error)}`, 'error')
    }
  }

  async function handleAggregateSkills() {
    if (gitBusy || !gitPath) {
      notify('请先选择本地同步目录', 'error')
      return
    }

    flushSync(() => setGitBusyAction('aggregate'))
    await waitForNextPaint()
    try {
      await syncToGit(gitPath)
      await refreshData()
      notify('已将所有应用中的 skills 汇总到本地同步目录', 'success')
    } catch (error) {
      notify(`汇总技能失败: ${String(error)}`, 'error')
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
      title: '选择技能同步目录',
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
      notify('本地同步目录已更新，当前技能已自动同步到新路径', 'success')
    } catch (error) {
      notify(`更新本地同步目录失败: ${String(error)}`, 'error')
    } finally {
      setGitBusyAction(null)
    }
  }

  async function handleSync() {
    if (gitBusy || !gitPath) {
      notify('请先选择本地同步目录', 'error')
      return
    }

    flushSync(() => setGitBusyAction('sync'))
    await waitForNextPaint()
    try {
      const message = await gitSync(gitPath)
      await refreshData()
      notify(message, 'success')
    } catch (error) {
      notify(`同步失败: ${String(error)}`, 'error')
    } finally {
      setGitBusyAction(null)
    }
  }

  async function handlePush() {
    if (gitBusy || !gitPath) {
      notify('请先选择本地同步目录', 'error')
      return
    }

    flushSync(() => setGitBusyAction('push'))
    await waitForNextPaint()
    try {
      await gitPush(gitPath)
      await refreshData()
      notify('已推送到远程仓库', 'success')
    } catch (error) {
      notify(`推送失败: ${String(error)}`, 'error')
    } finally {
      setGitBusyAction(null)
    }
  }

  async function handlePull() {
    if (gitBusy || !gitPath) {
      notify('请先选择本地同步目录', 'error')
      return
    }

    flushSync(() => setGitBusyAction('pull'))
    await waitForNextPaint()
    try {
      const message = await gitPull(gitPath)
      await refreshData()
      notify(message, 'success')
    } catch (error) {
      notify(`拉取失败: ${String(error)}`, 'error')
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
      notify('Git 配置已保存', 'success')
    } catch (error) {
      notify(`保存配置失败: ${String(error)}`, 'error')
    } finally {
      setGitBusyAction(null)
    }
  }

  async function chooseCustomPath(setter: (path: string) => void) {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: '选择路径',
    })

    if (typeof selected === 'string' && selected) {
      setter(selected)
    }
  }

  async function handleAddCustomApp() {
    if (!customAppName.trim() || !customAppPath.trim()) {
      notify('请填写应用名称和路径', 'error')
      return
    }

    try {
      await addCustomApp(customAppName.trim(), customAppPath.trim())
      setCustomAppName('')
      setCustomAppPath('')
      setCustomModalOpen(false)
      await refreshData()
      notify('自定义应用已添加', 'success')
    } catch (error) {
      notify(`添加失败: ${String(error)}`, 'error')
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
      notify('路径已更新', 'success')
    } catch (error) {
      notify(`更新路径失败: ${String(error)}`, 'error')
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

  async function handleConfirmRenameSkill() {
    if (!selectedSkill || !renameValue.trim()) {
      notify('请输入新的技能名称', 'error')
      return
    }

    setSkillBusy(true)
    try {
      await renameSkill(selectedSkill.path, renameValue.trim())
      setRenameModalOpen(false)
      setConflictModalOpen(false)
      setSelectedSkill(null)
      await refreshData()
      notify('技能已重命名', 'success')
    } catch (error) {
      notify(`重命名失败: ${String(error)}`, 'error')
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
      notify('技能已删除', 'success')
    } catch (error) {
      notify(`删除失败: ${String(error)}`, 'error')
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
            <h1>AI Skills Manager</h1>
            <p className="hero__text">{heroText}</p>
          </div>
        </div>
        <div className="hero__actions">
          <button className="button button--primary button--hero" type="button" onClick={() => void refreshData(true)} disabled={appLoading}>
            <Scan size={18} />
            {refreshButtonLabel}
          </button>
          <ThemeToggle />
          <Link className="button button--square" to="/settings" aria-label="设置">
            <Settings size={22} />
          </Link>
        </div>
      </header>

      <section className="stats-grid">
        <article className="surface stat-card">
          <span className="stat-card__label">检测到的应用</span>
          <strong>{stats.appCount}</strong>
        </article>
        <article className="surface stat-card">
          <span className="stat-card__label">技能文件总数</span>
          <strong>{gitSkillLoading ? '扫描中...' : stats.skillCount}</strong>
        </article>
        <article className="surface stat-card">
          <span className="stat-card__label">已链接应用</span>
          <strong>{stats.linkedCount}</strong>
        </article>
        <article className="surface stat-card">
          <span className="stat-card__label">冲突条目</span>
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
                    应用列表
                  </button>
                  <button className="tab" type="button" onClick={() => setActiveTab('skills')}>
                    技能管理
                    {stats.conflictCount ? <span className="tab__count tab__count--danger">{stats.conflictCount}</span> : null}
                  </button>
                </div>
                <button className="button button--ghost" type="button" onClick={() => setCustomModalOpen(true)}>
                  <FolderPlus size={16} />
                  添加自定义应用
                </button>
              </div>

              {sortedApps.length === 0 ? (
                <div className="surface empty-state">
                  <p>{appLoading ? '正在检测应用，请稍候...' : '当前没有发现任何应用，点击上方“重新扫描”后再试。'}</p>
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
                          ? '链接中...'
                          : '取消中...'
                        : null
                    }
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
                    应用列表
                  </button>
                  <button className="tab tab--active" type="button">
                    技能管理
                    {stats.conflictCount ? <span className="tab__count">{stats.conflictCount}</span> : null}
                  </button>
                </div>
                <label className="search-box">
                  <Search size={16} />
                  <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索技能..." />
                </label>
              </div>

              {stats.conflictCount ? (
                <div className="surface conflict-banner">
                  <div className="conflict-banner__body">
                    <strong>检测到 {stats.conflictCount} 个技能冲突</strong>
                    <p>相同名称的技能在多个应用中存在不同版本，请在菜单中查看详情并处理。</p>
                  </div>
                </div>
              ) : null}

              {filteredSkills.length === 0 ? (
                <div className="surface empty-state">
                  <p>{search ? '没有匹配的技能结果。' : skillLoading ? '技能正在后台分批扫描，结果会陆续显示。' : '当前没有可显示的技能文件。'}</p>
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
                    aria-label="打开本地同步目录"
                    title="打开文件夹"
                  >
                    <FolderOpen size={18} />
                  </button>
                </div>
                <span className="side-column__path-label">本地同步目录</span>
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
                  {gitBusyAction === 'pickPath' || gitBusyAction === 'changePath' ? '选择中...' : '更改路径'}
                </button>
                <button
                  className="button button--primary side-column__path-primary"
                  type="button"
                  onClick={() => void handleAggregateSkills()}
                  disabled={gitBusy}
                >
                  {gitBusyAction === 'aggregate' ? <RefreshCw size={17} className="spin" /> : <Archive size={17} />}
                  {gitBusyAction === 'aggregate' ? '汇总中...' : '汇总技能'}
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
              {gitBusyAction === 'pickPath' ? '选择中...' : '选择本地同步目录'}
            </button>
          ) : null}
        </aside>
      </main>

      <Modal open={customModalOpen} title="添加自定义应用" onClose={() => setCustomModalOpen(false)}>
        <div className="modal__body">
          <div className="field-group">
            <label htmlFor="custom-app-name">应用名称</label>
            <input
              id="custom-app-name"
              value={customAppName}
              onChange={(event) => setCustomAppName(event.target.value)}
              placeholder="例如 My Agent"
            />
          </div>
          <div className="field-group">
            <label htmlFor="custom-app-path">技能目录</label>
            <div className="inline-field">
              <input
                id="custom-app-path"
                value={customAppPath}
                onChange={(event) => setCustomAppPath(event.target.value)}
                placeholder="/path/to/skills"
              />
              <button className="button button--ghost" type="button" onClick={() => void chooseCustomPath(setCustomAppPath)}>
                浏览
              </button>
            </div>
          </div>
        </div>
        <footer className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => setCustomModalOpen(false)}>
            取消
          </button>
          <button className="button button--primary" type="button" onClick={() => void handleAddCustomApp()}>
            添加
          </button>
        </footer>
      </Modal>

      <Modal open={editModalOpen} title={editingApp ? `设置 ${editingApp.name} 路径` : '设置路径'} onClose={() => setEditModalOpen(false)}>
        <div className="modal__body">
          <div className="field-group">
            <label htmlFor="edit-app-path">技能目录</label>
            <div className="inline-field">
              <input
                id="edit-app-path"
                value={editPathValue}
                onChange={(event) => setEditPathValue(event.target.value)}
                placeholder="清空则恢复默认路径"
              />
              <button className="button button--ghost" type="button" onClick={() => void chooseCustomPath(setEditPathValue)}>
                浏览
              </button>
            </div>
          </div>
        </div>
        <footer className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => setEditPathValue('')}>
            恢复默认
          </button>
          <button className="button button--primary" type="button" onClick={() => void handleSavePath()}>
            保存
          </button>
        </footer>
      </Modal>

      <Modal open={detailModalOpen} title={selectedSkill?.name ?? '技能详情'} onClose={() => setDetailModalOpen(false)}>
        {selectedSkill ? (
          <>
            <div className="modal__body modal__body--stack">
              <div className="detail-grid">
                <div className="detail-field">
                  <span>描述</span>
                  <strong>{selectedSkill.description || '暂无描述'}</strong>
                </div>
                <div className="detail-field">
                  <span>来源</span>
                  <strong>{selectedSkill.sources.join(', ')}</strong>
                </div>
                <div className="detail-field">
                  <span>路径</span>
                  <strong className="detail-field__path">{selectedSkill.path}</strong>
                </div>
                <div className="detail-field">
                  <span>大小</span>
                  <strong>{(selectedSkill.size / 1024).toFixed(1)} KB</strong>
                </div>
                <div className="detail-field">
                  <span>文件数</span>
                  <strong>{selectedSkill.fileCount}</strong>
                </div>
                <div className="detail-field">
                  <span>修改时间</span>
                  <strong>{selectedSkill.modified}</strong>
                </div>
              </div>
            </div>
            <footer className="modal__footer">
              <button className="button button--ghost" type="button" onClick={() => setDetailModalOpen(false)}>
                关闭
              </button>
              <button className="button button--primary" type="button" onClick={() => {
                setDetailModalOpen(false)
                handleStartRenameSkill(selectedSkill)
              }}>
                重命名
              </button>
            </footer>
          </>
        ) : null}
      </Modal>

      <Modal open={renameModalOpen} title={selectedSkill ? `重命名 ${selectedSkill.name}` : '重命名技能'} onClose={() => setRenameModalOpen(false)}>
        <div className="modal__body">
          <div className="field-group">
            <label htmlFor="rename-skill-name">技能名称</label>
            <input
              id="rename-skill-name"
              value={renameValue}
              onChange={(event) => setRenameValue(event.target.value)}
              placeholder="输入新的技能名称"
            />
          </div>
        </div>
        <footer className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => setRenameModalOpen(false)}>
            取消
          </button>
          <button className="button button--primary" type="button" onClick={() => void handleConfirmRenameSkill()} disabled={skillBusy}>
            保存
          </button>
        </footer>
      </Modal>

      <Modal open={deleteModalOpen} title={selectedSkill ? `删除 ${selectedSkill.name}` : '删除技能'} onClose={() => setDeleteModalOpen(false)}>
        <div className="modal__body modal__body--stack">
          <p className="modal__copy">删除后将移除技能文件或目录，此操作不可撤销。</p>
          {selectedSkill ? (
            <div className="detail-field">
              <span>目标路径</span>
              <strong className="detail-field__path">{selectedSkill.path}</strong>
            </div>
          ) : null}
        </div>
        <footer className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => setDeleteModalOpen(false)}>
            取消
          </button>
          <button className="button button--primary button--danger" type="button" onClick={() => void handleConfirmDeleteSkill()} disabled={skillBusy}>
            删除
          </button>
        </footer>
      </Modal>

      <Modal open={conflictModalOpen} title={selectedSkill ? `${selectedSkill.name} 存在冲突` : '技能冲突'} onClose={() => setConflictModalOpen(false)}>
        <div className="modal__body modal__body--stack">
          {selectedSkill ? (
            <>
              <p className="modal__copy">
                这个技能在多个应用中存在不同版本。当前检测到 {selectedSkill.duplicateCount} 份来源，涉及 {selectedSkill.sources.join('、')}。
              </p>
              <div className="detail-grid">
                <div className="detail-field">
                  <span>规范名</span>
                  <strong>{selectedSkill.canonicalName}</strong>
                </div>
                <div className="detail-field">
                  <span>路径</span>
                  <strong className="detail-field__path">{selectedSkill.path}</strong>
                </div>
              </div>
            </>
          ) : null}
        </div>
        <footer className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => setConflictModalOpen(false)}>
            稍后处理
          </button>
          <button className="button button--ghost" type="button" onClick={() => {
            setConflictModalOpen(false)
            if (selectedSkill) {
              handleAskDeleteSkill(selectedSkill)
            }
          }}>
            删除当前项
          </button>
          <button className="button button--primary" type="button" onClick={() => {
            setConflictModalOpen(false)
            if (selectedSkill) {
              handleStartRenameSkill(selectedSkill)
            }
          }}>
            重命名当前项
          </button>
        </footer>
      </Modal>

      <Modal open={confirmGitPathOpen} title="确认更改本地同步目录" onClose={() => {
        setConfirmGitPathOpen(false)
        setPendingGitPath('')
      }}>
        <div className="modal__body modal__body--stack">
          <p className="modal__copy">确定要把所有 skill 都同步到这个目录进行统一管理吗？</p>
          {pendingGitPath ? (
            <div className="detail-field">
              <span>目标目录</span>
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
            取消
          </button>
          <button className="button button--primary" type="button" onClick={() => void handleConfirmGitFolderChange()} disabled={gitBusyAction === 'changePath'}>
            {gitBusyAction === 'changePath' ? '同步中...' : '确定并同步'}
          </button>
        </footer>
      </Modal>
    </div>
  )
}
