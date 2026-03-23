import type { PropsWithChildren } from 'react'

interface ModalProps extends PropsWithChildren {
  open: boolean
  title: string
  onClose: () => void
}

export function Modal({ open, title, onClose, children }: ModalProps) {
  if (!open) {
    return null
  }

  return (
    <div className="modal-backdrop" role="presentation" onClick={onClose}>
      <section
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="modal__header">
          <div className="modal__title-wrap">
            <h2>{title}</h2>
          </div>
          <button className="modal__close" type="button" onClick={onClose} aria-label="关闭">
            ×
          </button>
        </header>
        {children}
      </section>
    </div>
  )
}
