import { Check, FolderOpen, PencilLine, Play, RefreshCw } from 'lucide-react'
import type { AppRecord } from '../types'
import { AnimatedNumber } from './AnimatedNumber'
import { AppBrandIcon } from './AppBrandIcon'

interface ApplicationCardProps {
  app: AppRecord
  totalSkillCount: number
  busy: boolean
  busyLabel?: string | null
  onToggleLink: (app: AppRecord) => void
  onOpenFolder: (app: AppRecord) => void
  onLaunchApp: (app: AppRecord) => void
  onEditPath: (app: AppRecord) => void
}

export function ApplicationCard({
  app,
  totalSkillCount,
  busy,
  busyLabel,
  onToggleLink,
  onOpenFolder,
  onLaunchApp,
  onEditPath,
}: ApplicationCardProps) {
  const displaySkillCount = app.isLinked ? totalSkillCount : app.skillCount

  return (
    <article className={`surface card-app ${busy ? 'card-app--busy' : ''}`}>
      {busy ? (
        <div className="card-app__overlay" aria-live="polite" aria-busy="true">
          <RefreshCw size={18} className="spin" />
          <span>{busyLabel ?? '正在处理...'}</span>
        </div>
      ) : null}
      <div className="card-app__header">
        <div className="card-app__identity">
          <div className="card-app__icon-wrap">
            <AppBrandIcon appId={app.id} appName={app.name} />
          </div>
          <div className="card-app__text">
            <h3 className="card-app__title">{app.name}</h3>
            <p className="card-app__path ellipsis">{app.path}</p>
          </div>
        </div>
        <div className="card-app__header-tools">
          <div className="card-app__hover-actions">
            <button
              className="button button--icon-ghost card-app__action"
              type="button"
              onClick={() => onOpenFolder(app)}
              disabled={busy}
              aria-label={`打开 ${app.name} 文件夹`}
              title="打开文件夹"
            >
              <FolderOpen size={18} />
            </button>
            <button
              className="button button--icon-ghost card-app__action"
              type="button"
              onClick={() => onLaunchApp(app)}
              disabled={busy || !app.isInstalled}
              aria-label={`运行 ${app.name}`}
              title="运行软件"
            >
              <Play size={18} />
            </button>
            <button
              className="button button--icon-ghost card-app__action"
              type="button"
              onClick={() => onEditPath(app)}
              disabled={busy}
              aria-label={`修改 ${app.name} 路径`}
              title="修改文件夹"
            >
              <PencilLine size={18} />
            </button>
          </div>
          <div className="card-app__badges">
            {app.isInstalled ? (
              <span className="badge badge--detected">
                <Check className="badge__icon" size={14} />
                已检测
              </span>
            ) : (
              <span className="badge badge--muted">未安装</span>
            )}
          </div>
        </div>
      </div>

      <div className="card-app__details">
        <div>
          <div className="card-app__count">
            <AnimatedNumber
              value={displaySkillCount}
              className={`card-app__count-number ${app.isLinked ? 'card-app__count-number--linked' : ''}`}
            />
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
          <span className={`switch-pill__label ${app.isLinked ? 'switch-pill__label--linked' : ''} ${busy ? 'switch-pill__label--busy' : ''}`}>
            {busy ? busyLabel ?? '处理中...' : app.isLinked ? '已链接' : '未链接'}
          </span>
          <span className="switch-pill__track">
            {busy ? <RefreshCw size={12} className="switch-pill__spinner spin" /> : <span className="switch-pill__thumb" />}
          </span>
        </button>
      </div>

      {app.isCustom ? (
        <button className="button button--card button--full" type="button" onClick={() => onEditPath(app)} disabled={busy}>
          <FolderOpen size={18} />
          选择路径加载技能
        </button>
      ) : null}
    </article>
  )
}
