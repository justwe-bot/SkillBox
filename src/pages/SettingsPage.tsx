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
  Languages,
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
import { formatDate } from '../lib/i18n'
import { useI18n } from '../lib/i18n-context'
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
import type { AppLanguage, AppPreferences, AppRecord, DownloadUpdateResult, UpdateCheckResult } from '../types'

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

function getPlatformLabel(fallbackLabel: string) {
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

  return fallbackLabel
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
  const { language, setLanguage, t } = useI18n()
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
  const platformLabel = useMemo(() => getPlatformLabel(t('common.desktop')), [t])
  const hasChanges =
    savedGitPath !== gitPathDraft ||
    savedPreferences.autoScan !== preferences.autoScan ||
    savedPreferences.autoSync !== preferences.autoSync ||
    savedPreferences.desktopNotifications !== preferences.desktopNotifications ||
    savedPreferences.theme !== preferences.theme ||
    savedPreferences.language !== preferences.language
  const hasDownloadedLatestUpdate =
    Boolean(downloadedUpdate) &&
    Boolean(updateResult?.latestVersion) &&
    downloadedUpdate?.version === updateResult?.latestVersion

  useEffect(() => {
    void loadSettings()

    return () => {
      applyTheme(loadPreferences().theme)
      setLanguage(loadPreferences().language)
    }
  }, [setLanguage])

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
      setLanguage(nextPreferences.language)
      setVersion(versionState)
    } catch (error) {
      notify(t('settings.notifications.loadFailed', { error: String(error) }), 'error')
    } finally {
      setLoading(false)
    }
  }

  function updatePreferences(next: AppPreferences) {
    setPreferences(next)
    applyTheme(next.theme)
    setLanguage(next.language)
  }

  function updateTheme(theme: AppPreferences['theme']) {
    updatePreferences({ ...preferences, theme })
  }

  function toggleHeaderTheme() {
    updateTheme(appliedTheme === 'dark' ? 'light' : 'dark')
  }

  function updateLanguage(nextLanguage: AppLanguage) {
    updatePreferences({ ...preferences, language: nextLanguage })
  }

  async function browseGitPath() {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: t('settings.paths.storageLabel'),
    })

    if (typeof selected === 'string' && selected) {
      setGitPathDraft(selected)
    }
  }

  async function browseCustomPath() {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: t('settings.dialog.addCustomPath'),
    })

    if (typeof selected === 'string' && selected) {
      setCustomAppPath(selected)
      if (!customAppName.trim()) {
        const parts = selected.split('/').filter(Boolean)
        const inferredName = parts[parts.length - 1] ?? t('common.customApp')
        setCustomAppName(inferredName)
      }
    }
  }

  async function handleSaveSettings() {
    if (saving) {
      return
    }

    if (!gitPathDraft.trim()) {
      notify(t('settings.notifications.chooseStoragePathFirst'), 'error')
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
      notify(t('settings.footer.saved'), 'success')
    } catch (error) {
      notify(t('settings.notifications.saveFailed', { error: String(error) }), 'error')
    } finally {
      setSaving(false)
    }
  }

  function handleResetSettings() {
    setPreferences(savedPreferences)
    setGitPathDraft(savedGitPath)
    applyTheme(savedPreferences.theme)
    setLanguage(savedPreferences.language)
    notify(t('settings.footer.resetSuccess'), 'success')
  }

  async function handleAddCustomApp() {
    if (!customAppPath.trim()) {
      notify(t('settings.notifications.chooseCustomPathFirst'), 'error')
      return
    }

    const pathParts = customAppPath.split('/').filter(Boolean)
    const resolvedName = customAppName.trim() || pathParts[pathParts.length - 1] || t('common.customApp')

    try {
      await addCustomApp(resolvedName, customAppPath.trim())
      setCustomModalOpen(false)
      setCustomAppName('')
      setCustomAppPath('')
      await loadSettings()
      notify(t('settings.notifications.customPathAdded'), 'success')
    } catch (error) {
      notify(t('settings.notifications.customPathAddFailed', { error: String(error) }), 'error')
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
        notify(t('settings.notifications.updateFound', { version: result.latestVersion }), 'success')
      } else {
        notify(t('settings.notifications.noUpdate'), 'success')
      }
    } catch (error) {
      notify(t('settings.notifications.checkUpdateFailed', { error: String(error) }), 'error')
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
      notify(t('settings.notifications.installerDownloaded', { fileName: result.fileName }), 'success')

      try {
        await openDownloadedUpdate(result.filePath)
      } catch (error) {
        notify(t('settings.notifications.installerOpenFailed', { error: String(error) }), 'error')
      }
    } catch (error) {
      notify(t('settings.notifications.downloadFailed', { error: String(error) }), 'error')
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
      notify(t('settings.notifications.openInstallerFailed', { error: String(error) }), 'error')
    }
  }

  async function handleOpenReleasePage() {
    try {
      await openExternal(updateResult?.releaseUrl || releasesUrl)
    } catch (error) {
      notify(t('settings.notifications.openReleaseFailed', { error: String(error) }), 'error')
    }
  }

  return (
    <div className="page-shell">
      <header className="hero hero--settings">
        <div className="hero__brand hero__brand--settings">
          <Link className="button button--square button--back" to="/" aria-label={t('settings.backToDashboard')}>
            <ArrowLeft size={18} />
          </Link>
          <FigmaSkillIcon className="hero__settings-icon" size={52} />
          <div>
            <h1>{t('settings.title')}</h1>
            <p className="hero__text">{t('settings.subtitle')}</p>
          </div>
        </div>
        <div className="hero__actions">
          <button
            className="button button--square button--theme"
            type="button"
            onClick={toggleHeaderTheme}
            aria-label={t('theme.toggle')}
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
                <h2>{t('settings.paths.title')}</h2>
                <p className="muted">{t('settings.paths.description')}</p>
              </div>
            </div>
          </div>

          <div className="settings-panel__body">
            <div className="settings-field">
              <div className="settings-field__label">{t('settings.paths.storageLabel')}</div>
              <div className="settings-input-row">
                <input
                  className="settings-input"
                  value={gitPathDraft}
                  readOnly
                  placeholder={t('settings.paths.storagePlaceholder')}
                />
                <button className="button button--ghost settings-input-row__action" type="button" onClick={browseGitPath}>
                  {t('common.browse')}
                </button>
              </div>
              <p className="settings-help">{t('settings.paths.storageHelp')}</p>
            </div>

            <div className="settings-separator" />

            <div className="settings-subsection">
              <div className="settings-subsection__header">
                <div>
                  <h3>{t('settings.paths.scanTitle')}</h3>
                  <p className="muted">{t('settings.paths.scanDescription')}</p>
                </div>
                <button className="button button--ghost settings-subsection__action" type="button" onClick={() => setCustomModalOpen(true)}>
                  <Plus size={16} />
                  {t('settings.paths.addCustomPath')}
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
                          {t('settings.paths.detected')}
                        </>
                      ) : (
                        t('settings.paths.notDetected')
                      )}
                    </span>
                  </div>
                ))}

                {!apps.length && !loading ? <div className="empty-state">{t('settings.paths.empty')}</div> : null}
              </div>
            </div>
          </div>
        </section>

        <section className="surface settings-panel">
          <div className="settings-panel__header">
            <div className="settings-panel__title">
              <Bell size={18} />
              <div>
                <h2>{t('settings.automation.title')}</h2>
                <p className="muted">{t('settings.automation.description')}</p>
              </div>
            </div>
          </div>

          <div className="settings-panel__body settings-list">
            <label className="settings-switch-row">
              <div>
                <strong>{t('settings.automation.autoScanTitle')}</strong>
                <p className="muted">{t('settings.automation.autoScanDescription')}</p>
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
                <strong>{t('settings.automation.autoSyncTitle')}</strong>
                <p className="muted">{t('settings.automation.autoSyncDescription')}</p>
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
                <strong>{t('settings.automation.notificationsTitle')}</strong>
                <p className="muted">{t('settings.automation.notificationsDescription')}</p>
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
              <Languages size={18} />
              <div>
                <h2>{t('settings.language.title')}</h2>
                <p className="muted">{t('settings.language.description')}</p>
              </div>
            </div>
          </div>

          <div className="settings-panel__body">
            <div className="settings-theme-grid">
              {[
                { value: 'zh-CN' as const, label: t('settings.language.zh'), description: t('settings.language.zhDescription') },
                { value: 'en-US' as const, label: t('settings.language.en'), description: t('settings.language.enDescription') },
              ].map((option) => {
                const active = preferences.language === option.value

                return (
                  <button
                    key={option.value}
                    className={`settings-theme-choice ${active ? 'settings-theme-choice--active' : ''}`}
                    type="button"
                    onClick={() => updateLanguage(option.value)}
                  >
                    <span className="settings-theme-choice__icon">
                      <Languages size={16} />
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
              <Palette size={18} />
              <div>
                <h2>{t('settings.appearance.title')}</h2>
                <p className="muted">{t('settings.appearance.description')}</p>
              </div>
            </div>
          </div>

          <div className="settings-panel__body">
            <div className="settings-theme-grid">
              {[
                { value: 'light' as const, label: t('settings.appearance.light'), description: t('settings.appearance.lightDescription'), icon: Sun },
                { value: 'dark' as const, label: t('settings.appearance.dark'), description: t('settings.appearance.darkDescription'), icon: Moon },
                { value: 'system' as const, label: t('settings.appearance.system'), description: t('settings.appearance.systemDescription'), icon: Monitor },
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
                <h2>{t('settings.about.title')}</h2>
                <p className="muted">{t('settings.about.description')}</p>
              </div>
            </div>
          </div>

          <div className="settings-panel__body settings-list">
            <div className="settings-info-row">
              <span>{t('common.version')}</span>
              <strong>{version}</strong>
            </div>
            <div className="settings-info-row">
              <span>{t('settings.about.githubLatest')}</span>
              <strong>{updateResult?.latestVersion ?? t('common.notChecked')}</strong>
            </div>
            <div className="settings-info-row">
              <span>{t('settings.about.updateStatus')}</span>
              <strong>{updateResult ? (updateResult.updateAvailable ? t('settings.about.updateAvailable') : t('settings.about.upToDate')) : t('common.notChecked')}</strong>
            </div>
            <div className="settings-info-row">
              <span>{t('settings.about.publishedAt')}</span>
              <strong>{formatDate(updateResult?.publishedAt ?? null, language)}</strong>
            </div>
            <div className="settings-info-row">
              <span>{t('common.platform')}</span>
              <strong>{platformLabel}</strong>
            </div>
            <div className="settings-info-row">
              <span>{t('settings.about.downloadedUpdate')}</span>
              <strong>{downloadedUpdate?.fileName ?? t('common.notDownloaded')}</strong>
            </div>
            <div className="settings-info-row">
              <span>{t('common.license')}</span>
              <strong>MIT</strong>
            </div>
            {updateResult?.notes ? (
              <div className="settings-update-note">
                <span>{t('settings.about.releaseNotes')}</span>
                <p>{updateResult.notes}</p>
              </div>
            ) : null}
            {downloadProgress && downloadingUpdate ? (
              <div className="settings-update-progress" aria-live="polite" aria-busy="true">
                <div className="settings-update-progress__header">
                  <span>{downloadProgress.status === 'preparing' ? t('settings.about.preparingDownload') : t('settings.about.downloadingFile', { fileName: downloadProgress.fileName })}</span>
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
                  <span>{downloadProgress.totalBytes ? `${downloadProgress.percentage.toFixed(0)}%` : t('settings.about.fetchingSize')}</span>
                </div>
              </div>
            ) : null}
            <div className="settings-actions-row">
              <button className="button button--ghost settings-repo-button" type="button" onClick={() => void handleCheckUpdates()} disabled={checkingUpdates || downloadingUpdate}>
                <RefreshCcw size={16} />
                {checkingUpdates ? t('settings.about.checking') : t('settings.about.checkForUpdates')}
              </button>
              {updateResult?.updateAvailable ? (
                <button className="button button--primary settings-repo-button" type="button" onClick={() => void handleDownloadUpdate()} disabled={checkingUpdates || downloadingUpdate}>
                  <Download size={16} />
                  {downloadingUpdate ? t('settings.about.downloading') : hasDownloadedLatestUpdate ? t('settings.about.redownloadUpdate') : t('settings.about.downloadUpdate')}
                </button>
              ) : null}
              {downloadedUpdate ? (
                <button className="button button--ghost settings-repo-button" type="button" onClick={() => void handleOpenDownloadedInstaller()} disabled={downloadingUpdate}>
                  <FolderOpen size={16} />
                  {t('settings.about.openInstaller')}
                </button>
              ) : null}
              <button className="button button--ghost settings-repo-button" type="button" onClick={() => void handleOpenReleasePage()}>
                <ExternalLink size={16} />
                {t('settings.about.openReleases')}
              </button>
              <button
                className="button button--ghost settings-repo-button"
                type="button"
                onClick={() => void openExternal(repoUrl)}
              >
                <ExternalLink size={16} />
                {t('settings.about.openRepository')}
              </button>
            </div>
          </div>
        </section>

        <div className="settings-footer">
          <button className="button button--ghost settings-footer__button" type="button" onClick={handleResetSettings} disabled={!hasChanges || saving}>
            <RefreshCcw size={16} />
            {t('settings.footer.reset')}
          </button>
          <button className="button button--primary settings-footer__button" type="button" onClick={handleSaveSettings} disabled={!hasChanges || saving}>
            <Save size={16} />
            {saving ? t('common.saving') : t('settings.footer.saveSettings')}
          </button>
        </div>
      </main>

      <Modal open={customModalOpen} title={t('settings.dialog.addCustomPath')} onClose={() => setCustomModalOpen(false)}>
        <div className="modal__body">
          <div className="field-group">
            <label htmlFor="custom-app-name">{t('common.appName')}</label>
            <input
              id="custom-app-name"
              value={customAppName}
              onChange={(event) => setCustomAppName(event.target.value)}
              placeholder={t('settings.dialog.customAppNamePlaceholder')}
            />
          </div>
          <div className="field-group">
            <label htmlFor="custom-app-path">{t('common.path')}</label>
            <div className="inline-field">
              <input
                id="custom-app-path"
                value={customAppPath}
                readOnly
                placeholder={t('settings.dialog.customAppPathPlaceholder')}
              />
              <button className="button button--ghost" type="button" onClick={() => void browseCustomPath()}>
                <Folder size={16} />
                {t('common.browse')}
              </button>
            </div>
          </div>
        </div>
        <div className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => setCustomModalOpen(false)}>
            {t('common.cancel')}
          </button>
          <button className="button button--primary" type="button" onClick={() => void handleAddCustomApp()}>
            {t('settings.dialog.addPath')}
          </button>
        </div>
      </Modal>
    </div>
  )
}
