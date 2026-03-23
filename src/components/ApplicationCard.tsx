import { FolderOpen } from 'lucide-react'
import type { AppRecord } from '../types'

interface ApplicationCardProps {
  app: AppRecord
  busy: boolean
  onToggleLink: (app: AppRecord) => void
  onEditPath: (app: AppRecord) => void
}

export function ApplicationCard({ app, busy, onToggleLink, onEditPath }: ApplicationCardProps) {
  return (
    <article className="surface card-app">
      <div className="card-app__header">
        <div className="card-app__identity">
          <div className="card-app__icon-wrap">
            <span className="card-app__emoji-icon">{app.icon}</span>
          </div>
          <div>
            <h3>{app.name}</h3>
            <p className="muted ellipsis">{app.path}</p>
          </div>
        </div>
        <div className="card-app__badges">
          {app.isInstalled ? (
            <span className="badge badge--detected">已检测</span>
          ) : (
            <span className="badge badge--muted">未安装</span>
          )}
        </div>
      </div>

      <div className="card-app__details">
        <div>
          <div className="card-app__count">
            <span className="card-app__count-number">{app.skillCount}</span>
            <span className="card-app__count-unit">个技能</span>
          </div>
        </div>
        <button
          className={`switch-pill ${app.isLinked ? 'switch-pill--on' : ''}`}
          type="button"
          onClick={() => onToggleLink(app)}
          disabled={busy || !app.isInstalled}
          aria-pressed={app.isLinked}
        >
          <span className="switch-pill__label">{app.isLinked ? '已链接' : '未链接'}</span>
          <span className="switch-pill__track">
            <span className="switch-pill__thumb" />
          </span>
        </button>
      </div>

      <button className="button button--card button--full" type="button" onClick={() => onEditPath(app)}>
        <FolderOpen size={18} />
        选择路径加载技能
      </button>
    </article>
  )
}
