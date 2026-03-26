import { Download, GitBranch, RefreshCw, Upload } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useI18n } from '../lib/i18n-context'
import type { GitSyncConfig } from '../types'

type GitBusyAction = 'saveConfig' | 'push' | 'pull' | 'sync' | 'aggregate' | 'pickPath' | 'changePath' | null

interface GitPanelProps {
  gitPath: string
  gitConfig: GitSyncConfig
  busyAction: GitBusyAction
  logs: string[]
  pushTitle: string
  pullTitle: string
  syncTitle: string
  onSaveConfig: (config: GitSyncConfig) => void
  onPush: () => void
  onPull: () => void
  onSync: () => void
}

export function GitPanel({ gitPath, gitConfig, busyAction, logs, pushTitle, pullTitle, syncTitle, onSaveConfig, onPush, onPull, onSync }: GitPanelProps) {
  const { t } = useI18n()
  const [repoUrl, setRepoUrl] = useState(gitConfig.repoUrl)
  const [branch, setBranch] = useState(gitConfig.branch)

  useEffect(() => {
    setRepoUrl(gitConfig.repoUrl)
    setBranch(gitConfig.branch)
  }, [gitConfig])

  const configured = Boolean(gitConfig.repoUrl.trim())
  const canRunGitActions = Boolean(gitPath && gitConfig.repoUrl.trim())
  const gitBusy = busyAction !== null
  const saveBusy = busyAction === 'saveConfig'
  const pushBusy = busyAction === 'push'
  const pullBusy = busyAction === 'pull'
  const syncBusy = busyAction === 'sync'

  return (
    <section className="surface side-panel side-panel--git">
      <div className="side-panel__header">
        <div className="side-panel__title">
          <GitBranch size={26} />
          <h3>{t('git.title')}</h3>
        </div>
        <span className={`badge ${configured ? 'badge--success' : 'badge--muted'} badge--compact`}>
          {configured ? t('git.configured') : t('git.notConfigured')}
        </span>
      </div>

      <div className="git-panel__body">
        {gitBusy ? (
          <div className="git-panel__overlay" aria-live="polite" aria-busy="true">
            <RefreshCw size={20} className="spin" />
            <span>正在操作...</span>
          </div>
        ) : null}
        <div className="field-group">
          <label htmlFor="git-path">{t('git.repoUrl')}</label>
          <input
            id="git-path"
            value={repoUrl}
            onChange={(event) => setRepoUrl(event.target.value)}
            placeholder="https://github.com/username/skills-repo.git"
            disabled={gitBusy}
          />
        </div>

        <div className="field-group">
          <label htmlFor="git-branch">{t('git.branch')}</label>
          <input id="git-branch" value={branch} onChange={(event) => setBranch(event.target.value)} placeholder="main" disabled={gitBusy} />
        </div>

        <button
          className="button button--card button--full git-panel__save"
          type="button"
          onClick={() => onSaveConfig({ repoUrl, branch })}
          disabled={gitBusy}
        >
          {saveBusy ? (
            <>
              <RefreshCw size={16} className="spin" />
              {t('git.saveConfigBusy')}
            </>
          ) : (
            t('git.saveConfig')
          )}
        </button>

        <div className="side-panel__divider" />

        {!gitPath ? <p className="git-panel__hint">{t('git.selectDirectoryHint')}</p> : null}

        <div className="git-panel__actions">
          <button className="button button--primary git-action" type="button" onClick={onPush} disabled={gitBusy || !canRunGitActions} title={pushTitle}>
            {pushBusy ? <RefreshCw size={17} className="spin" /> : <Upload size={17} />}
            {pushBusy ? t('git.pushBusy') : t('git.push')}
          </button>
          <button className="button button--card git-action" type="button" onClick={onPull} disabled={gitBusy || !canRunGitActions} title={pullTitle}>
            {pullBusy ? <RefreshCw size={17} className="spin" /> : <Download size={17} />}
            {pullBusy ? t('git.pullBusy') : t('git.pull')}
          </button>
          <button className="button button--card git-action" type="button" onClick={onSync} disabled={gitBusy || !canRunGitActions} title={syncTitle}>
            <RefreshCw size={17} className={syncBusy ? 'spin' : ''} />
            {syncBusy ? t('git.syncBusy') : t('git.sync')}
          </button>
        </div>

        {logs.length > 0 && (
          <div className="git-panel__logs">
            <div className="git-panel__logs-header">操作日志</div>
            <div className="git-panel__logs-content">
              {logs.map((log, index) => (
                <div key={index} className="git-panel__log-line">{log}</div>
              ))}
            </div>
          </div>
        )}
      </div>
    </section>
  )
}
