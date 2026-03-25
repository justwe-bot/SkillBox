import type { PropsWithChildren } from 'react'
import { useI18n } from '../lib/i18n-context'

interface ModalProps extends PropsWithChildren {
  open: boolean
  title: string
  onClose: () => void
  className?: string
}

export function Modal({ open, title, onClose, className, children }: ModalProps) {
  const { t } = useI18n()

  if (!open) {
    return null
  }

  return (
    <div className="modal-backdrop" role="presentation" onClick={onClose}>
      <section
        className={className ? `modal ${className}` : 'modal'}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="modal__header">
          <div className="modal__title-wrap">
            <h2>{title}</h2>
          </div>
          <button className="modal__close" type="button" onClick={onClose} aria-label={t('modal.close')}>
            ×
          </button>
        </header>
        {children}
      </section>
    </div>
  )
}
