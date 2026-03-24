import { Download, GitBranch, RefreshCw, Upload } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { GitSyncConfig } from '../types'

type GitBusyAction = 'saveConfig' | 'push' | 'pull' | 'sync' | 'aggregate' | 'pickPath' | 'changePath' | null

interface GitPanelProps {
  gitPath: string
  gitConfig: GitSyncConfig
  busyAction: GitBusyAction
  onSaveConfig: (config: GitSyncConfig) => void
  onPush: () => void
  onPull: () => void
  onSync: () => void
}

export function GitPanel({ gitPath, gitConfig, busyAction, onSaveConfig, onPush, onPull, onSync }: GitPanelProps) {
  const [repoUrl, setRepoUrl] = useState(gitConfig.repoUrl)
  const [username, setUsername] = useState(gitConfig.username)
  const [branch, setBranch] = useState(gitConfig.branch)

  useEffect(() => {
    setRepoUrl(gitConfig.repoUrl)
    setUsername(gitConfig.username)
    setBranch(gitConfig.branch)
  }, [gitConfig])

  const configured = Boolean(gitConfig.repoUrl.trim())
  const canRunGitActions = Boolean(gitPath && gitConfig.repoUrl.trim())
  const gitBusy = busyAction !== null
  const saveBusy = busyAction === 'saveConfig'
  const pushBusy = busyAction === 'push'
  const pullBusy = busyAction === 'pull'
  const syncBusy = busyAction === 'sync'
  const busyLabel =
    busyAction === 'saveConfig'
      ? '正在保存配置...'
      : busyAction === 'push'
        ? '正在推送到远程仓库...'
        : busyAction === 'pull'
          ? '正在从远程仓库拉取...'
          : busyAction === 'sync'
            ? '正在同步...'
            : '正在处理...'

  return (
    <section className="surface side-panel side-panel--git">
      <div className="side-panel__header">
        <div className="side-panel__title">
          <GitBranch size={26} />
          <h3>Git 同步</h3>
        </div>
        <span className={`badge ${configured ? 'badge--success' : 'badge--muted'} badge--compact`}>
          {configured ? '已配置' : '未配置'}
        </span>
      </div>

      <div className="git-panel__body">
        {gitBusy ? (
          <div className="git-panel__overlay" aria-live="polite" aria-busy="true">
            <RefreshCw size={20} className="spin" />
            <span>{busyLabel}</span>
          </div>
        ) : null}

        <div className="field-group">
          <label htmlFor="git-path">仓库地址</label>
          <input
            id="git-path"
            value={repoUrl}
            onChange={(event) => setRepoUrl(event.target.value)}
            placeholder="https://github.com/username/skills-repo.git"
            disabled={gitBusy}
          />
        </div>

        <div className="git-panel__row">
          <div className="field-group">
            <label htmlFor="git-username">用户名</label>
            <input
              id="git-username"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              placeholder="username"
              disabled={gitBusy}
            />
          </div>
          <div className="field-group">
            <label htmlFor="git-branch">分支</label>
            <input id="git-branch" value={branch} onChange={(event) => setBranch(event.target.value)} placeholder="main" disabled={gitBusy} />
          </div>
        </div>

        <button
          className="button button--card button--full git-panel__save"
          type="button"
          onClick={() => onSaveConfig({ repoUrl, username, branch })}
          disabled={gitBusy}
        >
          {saveBusy ? (
            <>
              <RefreshCw size={16} className="spin" />
              保存中...
            </>
          ) : (
            '保存配置'
          )}
        </button>

        <div className="side-panel__divider" />

        {!gitPath ? <p className="git-panel__hint">先选择本地同步目录，再执行推送、拉取或同步。</p> : null}

        <div className="git-panel__actions">
          <button className="button button--primary git-action" type="button" onClick={onPush} disabled={gitBusy || !canRunGitActions}>
            {pushBusy ? <RefreshCw size={17} className="spin" /> : <Upload size={17} />}
            {pushBusy ? '推送中' : '推送'}
          </button>
          <button className="button button--card git-action" type="button" onClick={onPull} disabled={gitBusy || !canRunGitActions}>
            {pullBusy ? <RefreshCw size={17} className="spin" /> : <Download size={17} />}
            {pullBusy ? '拉取中' : '拉取'}
          </button>
          <button className="button button--card git-action" type="button" onClick={onSync} disabled={gitBusy || !canRunGitActions}>
            <RefreshCw size={17} className={syncBusy ? 'spin' : ''} />
            {syncBusy ? '同步中' : '同步'}
          </button>
        </div>
      </div>
    </section>
  )
}
