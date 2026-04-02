import type { ReactNode } from 'react'
import { ArrowDownToLine, Download, RefreshCw, Star, Terminal, Trash2, X } from 'lucide-react'
import type { MarketSkillDetail, MarketSkillRecord } from '../types'
import { useI18n } from '../lib/i18n-context'

interface MarketSkillDetailModalProps {
  open: boolean
  skill: MarketSkillRecord | null
  detail: MarketSkillDetail | null
  loading: boolean
  error: string | null
  installed: boolean
  busy: boolean
  busyAction: 'install' | 'remove' | null
  onClose: () => void
  onInstall: () => void
  onRemove: () => void
  onOpenRepository: () => void
}

function stripMarkdownFenceLanguage(line: string) {
  return line.trim().slice(3).trim()
}

function parseInlineCode(text: string) {
  const segments = text.split(/(`[^`]+`)/g)
  return segments.map((segment, index) => {
    if (segment.startsWith('`') && segment.endsWith('`') && segment.length > 1) {
      return (
        <code key={`code-${index}`} className="market-detail-modal__inline-code">
          {segment.slice(1, -1)}
        </code>
      )
    }

    return segment
  })
}

function renderMarkdown(readme: string) {
  const normalized = readme.replace(/\r\n/g, '\n').trim()
  if (!normalized) {
    return null
  }

  const lines = normalized.split('\n')
  const nodes: ReactNode[] = []
  let index = 0

  while (index < lines.length) {
    const line = lines[index]
    const trimmed = line.trim()

    if (!trimmed) {
      index += 1
      continue
    }

    if (trimmed.startsWith('```')) {
      const codeLanguage = stripMarkdownFenceLanguage(trimmed)
      index += 1
      const codeLines: string[] = []
      while (index < lines.length && !lines[index].trim().startsWith('```')) {
        codeLines.push(lines[index])
        index += 1
      }
      if (index < lines.length) {
        index += 1
      }
      nodes.push(
        <pre key={`codeblock-${nodes.length}`} className="market-detail-modal__code-block">
          {codeLanguage ? <span className="market-detail-modal__code-language">{codeLanguage}</span> : null}
          <code>{codeLines.join('\n')}</code>
        </pre>,
      )
      continue
    }

    if (/^#{1,3}\s/.test(trimmed)) {
      const level = trimmed.match(/^#+/)?.[0].length ?? 1
      const content = trimmed.replace(/^#{1,3}\s+/, '')
      const className =
        level === 1
          ? 'market-detail-modal__markdown-h1'
          : level === 2
            ? 'market-detail-modal__markdown-h2'
            : 'market-detail-modal__markdown-h3'
      nodes.push(
        <div key={`heading-${nodes.length}`} className={className}>
          {parseInlineCode(content)}
        </div>,
      )
      index += 1
      continue
    }

    if (/^[-*]\s/.test(trimmed)) {
      const items: string[] = []
      while (index < lines.length && /^[-*]\s/.test(lines[index].trim())) {
        items.push(lines[index].trim().replace(/^[-*]\s+/, ''))
        index += 1
      }
      nodes.push(
        <ul key={`list-${nodes.length}`} className="market-detail-modal__list">
          {items.map((item, itemIndex) => (
            <li key={`list-item-${itemIndex}`}>{parseInlineCode(item)}</li>
          ))}
        </ul>,
      )
      continue
    }

    const paragraphLines: string[] = []
    while (index < lines.length) {
      const current = lines[index].trim()
      if (!current || current.startsWith('```') || /^#{1,3}\s/.test(current) || /^[-*]\s/.test(current)) {
        break
      }
      paragraphLines.push(current)
      index += 1
    }

    if (paragraphLines.length) {
      nodes.push(
        <p key={`paragraph-${nodes.length}`} className="market-detail-modal__paragraph">
          {parseInlineCode(paragraphLines.join(' '))}
        </p>,
      )
      continue
    }

    index += 1
  }

  return nodes
}

function localizeDownloadsLabel(downloadsLabel: string, locale: 'zh-CN' | 'en-US') {
  if (!downloadsLabel.trim()) {
    return downloadsLabel
  }

  if (locale === 'zh-CN') {
    return downloadsLabel.replace(/\s+installs?$/i, ' 下载')
  }

  return downloadsLabel
}

export function MarketSkillDetailModal({
  open,
  skill,
  detail,
  loading,
  error,
  installed,
  busy,
  busyAction,
  onClose,
  onInstall,
  onRemove,
  onOpenRepository,
}: MarketSkillDetailModalProps) {
  const { language, t } = useI18n()

  if (!open || !skill) {
    return null
  }

  const actionLabel = installed ? t('dashboard.market.removeSkill') : t('dashboard.market.installSkill')
  const actionBusyLabel =
    installed && busyAction === 'remove'
      ? t('dashboard.market.removing')
      : !installed && busyAction === 'install'
        ? t('dashboard.market.installing')
        : actionLabel

  return (
    <div className="market-detail-modal__backdrop" role="presentation" onClick={onClose}>
      <section
        className="market-detail-modal"
        role="dialog"
        aria-modal="true"
        aria-label={skill.name}
        onClick={(event) => event.stopPropagation()}
      >
        <button className="market-detail-modal__close" type="button" onClick={onClose} aria-label={t('modal.close')}>
          <X size={22} />
        </button>

        <div className="market-detail-modal__scroll">
          <div className="market-detail-modal__stack">
            <div className="market-detail-modal__hero">
              <div className="market-detail-modal__icon">
                <Terminal size={34} />
              </div>

              <div className="market-detail-modal__hero-copy">
                <h2>{skill.name}</h2>
                <div className="market-detail-modal__meta">
                  <span className="market-detail-modal__author">@{skill.author}</span>
                  <span>•</span>
                  <span className="market-detail-modal__meta-item">
                    <ArrowDownToLine size={14} />
                    {localizeDownloadsLabel(skill.downloadsLabel, language)}
                  </span>
                  {skill.ratingLabel ? (
                    <>
                      <span>•</span>
                      <span className="market-detail-modal__meta-item">
                        <Star size={14} className="market-detail-modal__star" />
                        {skill.ratingLabel} {t('dashboard.market.ratingSuffix')}
                      </span>
                    </>
                  ) : null}
                </div>
              </div>
            </div>

            {skill.description ? (
              <p className="market-detail-modal__description">{skill.description}</p>
            ) : null}

            <div className="market-detail-modal__repository">
              <span className="market-detail-modal__repository-label">{t('dashboard.market.repositoryLabel')}</span>
              <button className="market-detail-modal__repository-link" type="button" onClick={onOpenRepository}>
                {skill.repository}
              </button>
            </div>

            <div className="market-detail-modal__actions">
              <button
                className={installed ? 'market-detail-modal__action market-detail-modal__action--secondary' : 'market-detail-modal__action market-detail-modal__action--primary'}
                type="button"
                onClick={installed ? onRemove : onInstall}
                disabled={busy}
              >
                {busy ? <RefreshCw size={16} className="spin" /> : installed ? <Trash2 size={16} /> : <Download size={16} />}
                {actionBusyLabel}
              </button>
            </div>

            <div className="market-detail-modal__divider" />

            <div className="market-detail-modal__readme-section">
              <h3>{t('dashboard.market.readmeTitle')}</h3>
              <div className="market-detail-modal__readme-card">
                {loading ? (
                  <div className="market-detail-modal__loading">
                    <RefreshCw size={18} className="spin" />
                    <span>{t('dashboard.market.detailLoading')}</span>
                  </div>
                ) : error ? (
                  <div className="market-detail-modal__error">{error}</div>
                ) : detail?.readme ? (
                  <div className="market-detail-modal__markdown">{renderMarkdown(detail.readme)}</div>
                ) : (
                  <div className="market-detail-modal__empty">{t('dashboard.market.detailEmpty')}</div>
                )}
              </div>
            </div>
          </div>
        </div>
      </section>
    </div>
  )
}
