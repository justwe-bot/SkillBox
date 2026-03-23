import { Download, GitBranch, RefreshCw, Upload } from 'lucide-react'
import { useEffect, useState } from 'react'

interface GitPanelProps {
  gitPath: string
  syncing: boolean
  onSaveConfig: (config: { repoUrl: string; username: string; branch: string }) => void
  onSync: () => void
}

export function GitPanel({ gitPath, syncing, onSaveConfig, onSync }: GitPanelProps) {
  const [repoUrl, setRepoUrl] = useState(gitPath)
  const [username, setUsername] = useState('username')
  const [branch, setBranch] = useState('main')

  useEffect(() => {
    setRepoUrl(gitPath)
  }, [gitPath])

  return (
    <section className="surface side-panel">
      <div className="side-panel__header">
        <div className="side-panel__title">
          <GitBranch size={26} />
          <h3>Git 同步</h3>
        </div>
        <span className={`badge ${gitPath ? 'badge--success' : 'badge--muted'} badge--compact`}>
          {gitPath ? '已配置' : '未配置'}
        </span>
      </div>

      <div className="field-group">
        <label htmlFor="git-path">仓库地址</label>
        <input
          id="git-path"
          value={repoUrl}
          onChange={(event) => setRepoUrl(event.target.value)}
          placeholder="https://github.com/username/skills-repo.git"
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
          />
        </div>
        <div className="field-group">
          <label htmlFor="git-branch">分支</label>
          <input id="git-branch" value={branch} onChange={(event) => setBranch(event.target.value)} placeholder="main" />
        </div>
      </div>

      <button className="button button--card button--full" type="button" onClick={() => onSaveConfig({ repoUrl, username, branch })}>
        保存配置
      </button>

      <div className="side-panel__divider" />

      <div className="git-panel__actions">
        <button className="button button--primary git-action" type="button" onClick={onSync} disabled={syncing || !gitPath}>
          <Upload size={17} />
          推送
        </button>
        <button className="button button--card git-action" type="button" disabled>
          <Download size={17} />
          拉取
        </button>
        <button className="button button--card git-action" type="button" onClick={onSync} disabled={syncing || !gitPath}>
          <RefreshCw size={17} className={syncing ? 'spin' : ''} />
          同步
        </button>
      </div>
    </section>
  )
}
