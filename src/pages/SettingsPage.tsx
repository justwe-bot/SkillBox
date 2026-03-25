import { useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { open as openDialog } from '@tauri-apps/api/dialog'
import { listen } from '@tauri-apps/api/event'
import { open as openExternal } from '@tauri-apps/api/shell'
import {
  ArrowLeft,
  Bell,
  Check,
  Download,
  ExternalLink,
  Folder,
  FolderOpen,
  FolderRoot,
  Info,
  Monitor,
  Moon,
  Palette,
  Plus,
  RefreshCcw,
  Save,
  Sun,
} from 'lucide-react'
import { FigmaSkillIcon } from '../components/FigmaSkillIcon'
import { Modal } from '../components/Modal'
import { useToast } from '../components/ToastProvider'
import {
  addCustomApp,
  checkUpdates,
  downloadUpdate,
  getVersion,
  openDownloadedUpdate,
  saveGitPath,
  scanApps,
  updateDownloadProgressEvent,
} from '../lib/tauri'
import { loadPreferences, savePreferences } from '../lib/preferences'
import { applyTheme, resolveAppliedTheme } from '../lib/theme'
import type { AppPreferences, AppRecord, DownloadUpdateResult, UpdateCheckResult } from '../types'

const repoUrl = 'https://github.com/justwe-bot/SkillBox'
const releasesUrl = `${repoUrl}/releases`
const backgroundUpdateDismissKey = 'skillbox.dismissedUpdateVersion'

interface UpdateDownloadProgress {
  fileName: string
  downloadedBytes: number
  totalBytes: number | null
  percentage: number
  status: string
}

function getPlatformLabel() {
  const platform = `${navigator.platform ?? ''} ${navigator.userAgent ?? ''}`

  if (/mac/i.test(platform)) {
    return 'macOS'
  }

  if (/win/i.test(platform)) {
    return 'Windows'
  }

  if (/linux/i.test(platform)) {
    return 'Linux'
  }

  return 'Desktop'
}

function formatPublishedDate(value: string | null) {
  if (!value) {
    return '未检查'
  }

  const date = new Date(value)
  if (Number.isNaN(date.getTime())) {
    return value
  }

  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date)
}

function formatFileSize(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return '0 B'
  }

  if (bytes < 1024) {
    return `${bytes} B`
  }

  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`
  }

  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

export default function SettingsPage() {
  const { notify } = useToast()
  const initialPreferences = loadPreferences()
  const [savedPreferences, setSavedPreferences] = useState<AppPreferences>(initialPreferences)
  const [preferences, setPreferences] = useState<AppPreferences>(initialPreferences)
  const [version, setVersion] = useState('1.0.0')
  const [savedGitPath, setSavedGitPath] = useState('')
  const [gitPathDraft, setGitPathDraft] = useState('')
  const [apps, setApps] = useState<AppRecord[]>([])
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [customModalOpen, setCustomModalOpen] = useState(false)
  const [customAppName, setCustomAppName] = useState('')
  const [customAppPath, setCustomAppPath] = useState('')
  const [checkingUpdates, setCheckingUpdates] = useState(false)
  const [downloadingUpdate, setDownloadingUpdate] = useState(false)
  const [downloadedUpdate, setDownloadedUpdate] = useState<DownloadUpdateResult | null>(null)
  const [downloadProgress, setDownloadProgress] = useState<UpdateDownloadProgress | null>(null)
  const [updateResult, setUpdateResult] = useState<UpdateCheckResult | null>(null)

  const appliedTheme = resolveAppliedTheme(preferences.theme)
  const platformLabel = useMemo(() => getPlatformLabel(), [])
  const hasChanges =
    savedGitPath !== gitPathDraft ||
    savedPreferences.autoScan !== preferences.autoScan ||
    savedPreferences.autoSync !== preferences.autoSync ||
    savedPreferences.desktopNotifications !== preferences.desktopNotifications ||
    savedPreferences.theme !== preferences.theme
  const hasDownloadedLatestUpdate =
    Boolean(downloadedUpdate) &&
    Boolean(updateResult?.latestVersion) &&
    downloadedUpdate?.version === updateResult?.latestVersion

  useEffect(() => {
    void loadSettings()

    return () => {
      applyTheme(loadPreferences().theme)
    }
  }, [])

  useEffect(() => {
    let active = true
    let cleanup: (() => void) | undefined

    void listen<UpdateDownloadProgress>(updateDownloadProgressEvent, (event) => {
      if (active) {
        setDownloadProgress(event.payload)
      }
    }).then((unlisten) => {
      if (!active) {
        unlisten()
        return
      }

      cleanup = unlisten
    })

    return () => {
      active = false
      cleanup?.()
    }
  }, [])

  async function loadSettings() {
    setLoading(true)

    try {
      const [appState, versionState] = await Promise.all([
        scanApps(),
        getVersion().catch(() => '1.0.0'),
      ])
      const nextPreferences = loadPreferences()
      setApps(appState.apps)
      setSavedGitPath(appState.gitPath)
      setGitPathDraft(appState.gitPath)
      setSavedPreferences(nextPreferences)
      setPreferences(nextPreferences)
      setVersion(versionState)
    } catch (error) {
      notify(`加载设置失败: ${String(error)}`, 'error')
    } finally {
      setLoading(false)
    }
  }

  function updatePreferences(next: AppPreferences) {
    setPreferences(next)
    applyTheme(next.theme)
  }

  function updateTheme(theme: AppPreferences['theme']) {
    updatePreferences({ ...preferences, theme })
  }

  function toggleHeaderTheme() {
    updateTheme(appliedTheme === 'dark' ? 'light' : 'dark')
  }

  async function browseGitPath() {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: '选择技能存储目录',
    })

    if (typeof selected === 'string' && selected) {
      setGitPathDraft(selected)
    }
  }

  async function browseCustomPath() {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: '选择自定义应用路径',
    })

    if (typeof selected === 'string' && selected) {
      setCustomAppPath(selected)
      if (!customAppName.trim()) {
        const parts = selected.split('/').filter(Boolean)
        const inferredName = parts[parts.length - 1] ?? 'Custom App'
        setCustomAppName(inferredName)
      }
    }
  }

  async function handleSaveSettings() {
    if (saving) {
      return
    }

    if (!gitPathDraft.trim()) {
      notify('请先选择技能存储目录', 'error')
      return
    }

    setSaving(true)
    try {
      const trimmedGitPath = gitPathDraft.trim()
      await saveGitPath(trimmedGitPath)
      savePreferences(preferences)
      applyTheme(preferences.theme)
      setSavedGitPath(trimmedGitPath)
      setSavedPreferences(preferences)
      notify('设置已保存', 'success')
    } catch (error) {
      notify(`保存设置失败: ${String(error)}`, 'error')
    } finally {
      setSaving(false)
    }
  }

  function handleResetSettings() {
    setPreferences(savedPreferences)
    setGitPathDraft(savedGitPath)
    applyTheme(savedPreferences.theme)
    notify('已恢复未保存的设置', 'success')
  }

  async function handleAddCustomApp() {
    if (!customAppPath.trim()) {
      notify('请先选择自定义应用路径', 'error')
      return
    }

    const pathParts = customAppPath.split('/').filter(Boolean)
    const resolvedName = customAppName.trim() || pathParts[pathParts.length - 1] || 'Custom App'

    try {
      await addCustomApp(resolvedName, customAppPath.trim())
      setCustomModalOpen(false)
      setCustomAppName('')
      setCustomAppPath('')
      await loadSettings()
      notify('自定义扫描路径已添加', 'success')
    } catch (error) {
      notify(`添加自定义路径失败: ${String(error)}`, 'error')
    }
  }

  async function handleCheckUpdates() {
    if (checkingUpdates) {
      return
    }

    setCheckingUpdates(true)

    try {
      const result = await checkUpdates()
      setUpdateResult(result)
      setDownloadedUpdate((current) => {
        if (!current || !result.latestVersion) {
          return null
        }

        return current.version === result.latestVersion ? current : null
      })
      if (result.latestVersion && window.localStorage.getItem(backgroundUpdateDismissKey) !== result.latestVersion) {
        window.localStorage.removeItem(backgroundUpdateDismissKey)
      }

      if (result.updateAvailable && result.latestVersion) {
        notify(`发现新版本 ${result.latestVersion}`, 'success')
      } else {
        notify('当前已经是 GitHub 上的最新正式版本', 'success')
      }
    } catch (error) {
      notify(`检查更新失败: ${String(error)}`, 'error')
    } finally {
      setCheckingUpdates(false)
    }
  }

  async function handleDownloadUpdate() {
    if (downloadingUpdate) {
      return
    }

    setDownloadingUpdate(true)
    setDownloadProgress({
      fileName: downloadedUpdate?.fileName ?? updateResult?.releaseName ?? 'SkillBox',
      downloadedBytes: 0,
      totalBytes: null,
      percentage: 0,
      status: 'preparing',
    })

    try {
      const result = await downloadUpdate()
      setDownloadedUpdate(result)
      notify(`更新安装包已下载：${result.fileName}`, 'success')

      try {
        await openDownloadedUpdate(result.filePath)
      } catch (error) {
        notify(`安装包已下载，但自动打开失败: ${String(error)}`, 'error')
      }
    } catch (error) {
      notify(`下载更新失败: ${String(error)}`, 'error')
    } finally {
      setDownloadingUpdate(false)
    }
  }

  async function handleOpenDownloadedInstaller() {
    if (!downloadedUpdate) {
      return
    }

    try {
      await openDownloadedUpdate(downloadedUpdate.filePath)
    } catch (error) {
      notify(`打开安装包失败: ${String(error)}`, 'error')
    }
  }

  async function handleOpenReleasePage() {
    try {
      await openExternal(updateResult?.releaseUrl || releasesUrl)
    } catch (error) {
      notify(`打开发布页失败: ${String(error)}`, 'error')
    }
  }

  return (
    <div className="page-shell">
      <header className="hero hero--settings">
        <div className="hero__brand hero__brand--settings">
          <Link className="button button--square button--back" to="/" aria-label="返回总览">
            <ArrowLeft size={18} />
          </Link>
          <FigmaSkillIcon className="hero__settings-icon" size={52} />
          <div>
            <h1>设置</h1>
            <p className="hero__text">配置应用偏好设置</p>
          </div>
        </div>
        <div className="hero__actions">
          <button
            className="button button--square button--theme"
            type="button"
            onClick={toggleHeaderTheme}
            aria-label="切换主题"
          >
            {appliedTheme === 'dark' ? <Sun size={18} /> : <Moon size={18} />}
          </button>
        </div>
      </header>

      <main className="settings-shell">
        <section className="surface settings-panel">
          <div className="settings-panel__header">
            <div className="settings-panel__title">
              <FolderRoot size={18} />
              <div>
                <h2>路径设置</h2>
                <p className="muted">管理技能的本地存储位置和应用扫描来源。</p>
              </div>
            </div>
          </div>

          <div className="settings-panel__body">
            <div className="settings-field">
              <div className="settings-field__label">技能存储目录</div>
              <div className="settings-input-row">
                <input
                  className="settings-input"
                  value={gitPathDraft}
                  readOnly
                  placeholder="选择一个目录作为技能存储与 Git 同步目录"
                />
                <button className="button button--ghost settings-input-row__action" type="button" onClick={browseGitPath}>
                  浏览
                </button>
              </div>
              <p className="settings-help">所有扫描到的 skills 会统一同步到这个本地目录，作为 Git 管理的工作区。</p>
            </div>

            <div className="settings-separator" />

            <div className="settings-subsection">
              <div className="settings-subsection__header">
                <div>
                  <h3>应用扫描路径</h3>
                  <p className="muted">SkillBox 会按照应用预设目录和自定义路径查找技能。</p>
                </div>
                <button className="button button--ghost settings-subsection__action" type="button" onClick={() => setCustomModalOpen(true)}>
                  <Plus size={16} />
                  添加自定义路径
                </button>
              </div>

              <div className="settings-path-list">
                {apps.map((app) => (
                  <div className="settings-path-row" key={`${app.id}:${app.path}`}>
                    <div className="settings-path-row__content">
                      <strong>{app.name}</strong>
                      <span className="ellipsis">{app.customPath ?? app.path}</span>
                    </div>
                    <span className={`badge badge--compact ${app.isInstalled ? 'badge--success' : 'badge--muted'}`}>
                      {app.isInstalled ? (
                        <>
                          <Check className="badge__icon" size={14} />
                          已检测
                        </>
                      ) : (
                        '未检测'
                      )}
                    </span>
                  </div>
                ))}

                {!apps.length && !loading ? <div className="empty-state">暂时还没有可展示的应用路径。</div> : null}
              </div>
            </div>
          </div>
        </section>

        <section className="surface settings-panel">
          <div className="settings-panel__header">
            <div className="settings-panel__title">
              <Bell size={18} />
              <div>
                <h2>自动化设置</h2>
                <p className="muted">保持启动行为和同步习惯与 Figma 设计一致。</p>
              </div>
            </div>
          </div>

          <div className="settings-panel__body settings-list">
            <label className="settings-switch-row">
              <div>
                <strong>启动时自动扫描</strong>
                <p className="muted">打开 SkillBox 时自动刷新应用与技能列表。</p>
              </div>
              <button
                className={`settings-switch ${preferences.autoScan ? 'settings-switch--on' : ''}`}
                type="button"
                onClick={() => updatePreferences({ ...preferences, autoScan: !preferences.autoScan })}
                aria-pressed={preferences.autoScan}
              >
                <span className="settings-switch__thumb" />
              </button>
            </label>

            <label className="settings-switch-row">
              <div>
                <strong>自动 Git 同步</strong>
                <p className="muted">保存本地路径后自动准备同步目录，并在后续版本接入自动推送策略。</p>
              </div>
              <button
                className={`settings-switch ${preferences.autoSync ? 'settings-switch--on' : ''}`}
                type="button"
                onClick={() => updatePreferences({ ...preferences, autoSync: !preferences.autoSync })}
                aria-pressed={preferences.autoSync}
              >
                <span className="settings-switch__thumb" />
              </button>
            </label>

            <label className="settings-switch-row">
              <div>
                <strong>桌面通知</strong>
                <p className="muted">在扫描、同步或技能操作完成后给出桌面级提示。</p>
              </div>
              <button
                className={`settings-switch ${preferences.desktopNotifications ? 'settings-switch--on' : ''}`}
                type="button"
                onClick={() =>
                  updatePreferences({
                    ...preferences,
                    desktopNotifications: !preferences.desktopNotifications,
                  })
                }
                aria-pressed={preferences.desktopNotifications}
              >
                <span className="settings-switch__thumb" />
              </button>
            </label>
          </div>
        </section>

        <section className="surface settings-panel">
          <div className="settings-panel__header">
            <div className="settings-panel__title">
              <Palette size={18} />
              <div>
                <h2>外观设置</h2>
                <p className="muted">让浅色、深色和跟随系统三种模式与 Figma Make 的主题风格保持一致。</p>
              </div>
            </div>
          </div>

          <div className="settings-panel__body">
            <div className="settings-theme-grid">
              {[
                { value: 'light' as const, label: '浅色', description: '使用亮色界面。', icon: Sun },
                { value: 'dark' as const, label: '深色', description: '使用夜晚模式。', icon: Moon },
                { value: 'system' as const, label: '跟随系统', description: '自动跟随系统外观。', icon: Monitor },
              ].map((option) => {
                const Icon = option.icon
                const active = preferences.theme === option.value

                return (
                  <button
                    key={option.value}
                    className={`settings-theme-choice ${active ? 'settings-theme-choice--active' : ''}`}
                    type="button"
                    onClick={() => updateTheme(option.value)}
                  >
                    <span className="settings-theme-choice__icon">
                      <Icon size={16} />
                    </span>
                    <span className="settings-theme-choice__copy">
                      <strong>{option.label}</strong>
                      <span>{option.description}</span>
                    </span>
                  </button>
                )
              })}
            </div>
          </div>
        </section>

        <section className="surface settings-panel">
          <div className="settings-panel__header">
            <div className="settings-panel__title">
              <Info size={18} />
              <div>
                <h2>关于</h2>
                <p className="muted">当前应用信息与项目来源。</p>
              </div>
            </div>
          </div>

          <div className="settings-panel__body settings-list">
            <div className="settings-info-row">
              <span>版本</span>
              <strong>{version}</strong>
            </div>
            <div className="settings-info-row">
              <span>GitHub 最新版</span>
              <strong>{updateResult?.latestVersion ?? '未检查'}</strong>
            </div>
            <div className="settings-info-row">
              <span>更新状态</span>
              <strong>{updateResult ? (updateResult.updateAvailable ? '发现新版本' : '已是最新') : '未检查'}</strong>
            </div>
            <div className="settings-info-row">
              <span>发布时间</span>
              <strong>{formatPublishedDate(updateResult?.publishedAt ?? null)}</strong>
            </div>
            <div className="settings-info-row">
              <span>平台</span>
              <strong>{platformLabel}</strong>
            </div>
            <div className="settings-info-row">
              <span>已下载更新</span>
              <strong>{downloadedUpdate?.fileName ?? '未下载'}</strong>
            </div>
            <div className="settings-info-row">
              <span>许可证</span>
              <strong>MIT</strong>
            </div>
            {updateResult?.notes ? (
              <div className="settings-update-note">
                <span>Release 说明</span>
                <p>{updateResult.notes}</p>
              </div>
            ) : null}
            {downloadProgress && downloadingUpdate ? (
              <div className="settings-update-progress" aria-live="polite" aria-busy="true">
                <div className="settings-update-progress__header">
                  <span>{downloadProgress.status === 'preparing' ? '准备下载更新...' : `正在下载 ${downloadProgress.fileName}`}</span>
                  <strong>
                    {downloadProgress.totalBytes
                      ? `${formatFileSize(downloadProgress.downloadedBytes)} / ${formatFileSize(downloadProgress.totalBytes)}`
                      : formatFileSize(downloadProgress.downloadedBytes)}
                  </strong>
                </div>
                <div className="settings-update-progress__track">
                  <div
                    className="settings-update-progress__fill"
                    style={{ width: `${Math.max(downloadProgress.percentage, 4)}%` }}
                  />
                </div>
                <div className="settings-update-progress__footer">
                  <span>{downloadProgress.totalBytes ? `${downloadProgress.percentage.toFixed(0)}%` : '正在获取大小...'}</span>
                </div>
              </div>
            ) : null}
            <div className="settings-actions-row">
              <button className="button button--ghost settings-repo-button" type="button" onClick={() => void handleCheckUpdates()} disabled={checkingUpdates || downloadingUpdate}>
                <RefreshCcw size={16} />
                {checkingUpdates ? '检查中...' : '检查更新'}
              </button>
              {updateResult?.updateAvailable ? (
                <button className="button button--primary settings-repo-button" type="button" onClick={() => void handleDownloadUpdate()} disabled={checkingUpdates || downloadingUpdate}>
                  <Download size={16} />
                  {downloadingUpdate ? '下载中...' : hasDownloadedLatestUpdate ? '重新下载更新' : '下载更新'}
                </button>
              ) : null}
              {downloadedUpdate ? (
                <button className="button button--ghost settings-repo-button" type="button" onClick={() => void handleOpenDownloadedInstaller()} disabled={downloadingUpdate}>
                  <FolderOpen size={16} />
                  打开安装包
                </button>
              ) : null}
              <button className="button button--ghost settings-repo-button" type="button" onClick={() => void handleOpenReleasePage()}>
                <ExternalLink size={16} />
                打开 Releases
              </button>
              <button
                className="button button--ghost settings-repo-button"
                type="button"
                onClick={() => void openExternal(repoUrl)}
              >
                <ExternalLink size={16} />
                打开 GitHub 仓库
              </button>
            </div>
          </div>
        </section>

        <div className="settings-footer">
          <button className="button button--ghost settings-footer__button" type="button" onClick={handleResetSettings} disabled={!hasChanges || saving}>
            <RefreshCcw size={16} />
            重置
          </button>
          <button className="button button--primary settings-footer__button" type="button" onClick={handleSaveSettings} disabled={!hasChanges || saving}>
            <Save size={16} />
            {saving ? '保存中' : '保存设置'}
          </button>
        </div>
      </main>

      <Modal open={customModalOpen} title="添加自定义路径" onClose={() => setCustomModalOpen(false)}>
        <div className="modal__body">
          <div className="field-group">
            <label htmlFor="custom-app-name">应用名称</label>
            <input
              id="custom-app-name"
              value={customAppName}
              onChange={(event) => setCustomAppName(event.target.value)}
              placeholder="例如 Team Prompt Hub"
            />
          </div>
          <div className="field-group">
            <label htmlFor="custom-app-path">路径</label>
            <div className="inline-field">
              <input
                id="custom-app-path"
                value={customAppPath}
                readOnly
                placeholder="选择自定义技能目录"
              />
              <button className="button button--ghost" type="button" onClick={() => void browseCustomPath()}>
                <Folder size={16} />
                浏览
              </button>
            </div>
          </div>
        </div>
        <div className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => setCustomModalOpen(false)}>
            取消
          </button>
          <button className="button button--primary" type="button" onClick={() => void handleAddCustomApp()}>
            添加路径
          </button>
        </div>
      </Modal>
    </div>
  )
}
