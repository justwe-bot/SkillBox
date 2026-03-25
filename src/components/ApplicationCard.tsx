import { Check, FolderOpen, PencilLine, Play, RefreshCw } from 'lucide-react'
import type { AppRecord } from '../types'
import { useI18n } from '../lib/i18n-context'
import { AnimatedNumber } from './AnimatedNumber'
import { AppBrandIcon } from './AppBrandIcon'

interface ApplicationCardProps {
  app: AppRecord
  totalSkillCount: number
  busy: boolean
  busyLabel?: string | null
  onManageSkills: (app: AppRecord) => void
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
  onManageSkills,
  onToggleLink,
  onOpenFolder,
  onLaunchApp,
  onEditPath,
}: ApplicationCardProps) {
  const { t } = useI18n()
  const displaySkillCount = app.isLinked
    ? app.linkMode === 'managed'
      ? app.enabledSkillCount
      : totalSkillCount
    : app.skillCount

  return (
    <article className={`surface card-app ${busy ? 'card-app--busy' : ''}`}>
      {busy ? (
        <div className="card-app__overlay" aria-live="polite" aria-busy="true">
          <RefreshCw size={18} className="spin" />
          <span>{busyLabel ?? t('dashboard.card.processing')}</span>
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
              aria-label={t('dashboard.card.openFolder', { name: app.name })}
              title={t('dashboard.side.openFolder')}
            >
              <FolderOpen size={18} />
            </button>
            <button
              className="button button--icon-ghost card-app__action"
              type="button"
              onClick={() => onLaunchApp(app)}
              disabled={busy || !app.isInstalled}
              aria-label={t('dashboard.card.runApp', { name: app.name })}
              title={t('dashboard.card.runSoftware')}
            >
              <Play size={18} />
            </button>
            <button
              className="button button--icon-ghost card-app__action"
              type="button"
              onClick={() => onEditPath(app)}
              disabled={busy}
              aria-label={t('dashboard.card.editPath', { name: app.name })}
              title={t('dashboard.card.editFolder')}
            >
              <PencilLine size={18} />
            </button>
          </div>
          <div className="card-app__badges">
            {app.isInstalled ? (
              <span className="badge badge--detected">
                <Check className="badge__icon" size={14} />
                {t('dashboard.card.detected')}
              </span>
            ) : (
              <span className="badge badge--muted">{t('dashboard.card.notInstalled')}</span>
            )}
          </div>
        </div>
      </div>

      <div className="card-app__details">
        <div className="card-app__details-main">
          <div className="card-app__count">
            <AnimatedNumber
              value={displaySkillCount}
              className={`card-app__count-number ${app.isLinked ? 'card-app__count-number--linked' : ''}`}
            />
            <span className="card-app__count-unit">{t('dashboard.card.skillUnit')}</span>
          </div>
          {app.isLinked ? (
            <button className="button button--ghost card-app__manage-button" type="button" onClick={() => onManageSkills(app)} disabled={busy}>
              {t('dashboard.card.manageSkills')}
            </button>
          ) : null}
        </div>
        <button
          className={`switch-pill ${app.isLinked ? 'switch-pill--on' : ''}`}
          type="button"
          onClick={() => onToggleLink(app)}
          disabled={busy || !app.isInstalled}
          aria-pressed={app.isLinked}
        >
          <span className={`switch-pill__label ${app.isLinked ? 'switch-pill__label--linked' : ''} ${busy ? 'switch-pill__label--busy' : ''}`}>
            {busy ? busyLabel ?? t('dashboard.card.processing') : app.isLinked ? t('dashboard.card.linked') : t('dashboard.card.unlinked')}
          </span>
          <span className="switch-pill__track">
            {busy ? <RefreshCw size={12} className="switch-pill__spinner spin" /> : <span className="switch-pill__thumb" />}
          </span>
        </button>
      </div>

      {app.isCustom ? (
        <button className="button button--card button--full" type="button" onClick={() => onEditPath(app)} disabled={busy}>
          <FolderOpen size={18} />
          {t('dashboard.card.loadSkillsFromPath')}
        </button>
      ) : null}
    </article>
  )
}
