import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type PropsWithChildren,
} from 'react'

type ToastTone = 'success' | 'error' | 'info'
type ToastActionStyle = 'primary' | 'ghost' | 'danger'

const DEFAULT_TOAST_DURATION_MS = 3200
const ERROR_TOAST_DURATION_MS = 6500

interface ToastAction {
  label: string
  style?: ToastActionStyle
  onClick?: () => void | Promise<void>
}

interface ToastItem {
  id: number
  tone: ToastTone
  message: string
  actions?: ToastAction[]
}

interface ToastContextValue {
  notify: (message: string, tone?: ToastTone) => void
  notifyAction: (message: string, options?: { tone?: ToastTone; actions?: ToastAction[]; durationMs?: number | null }) => void
}

const ToastContext = createContext<ToastContextValue | null>(null)

export function ToastProvider({ children }: PropsWithChildren) {
  const [toasts, setToasts] = useState<ToastItem[]>([])
  const timeoutsRef = useRef<Map<number, number>>(new Map())

  const dismissToast = useCallback((id: number) => {
    const timeoutId = timeoutsRef.current.get(id)
    if (timeoutId) {
      window.clearTimeout(timeoutId)
      timeoutsRef.current.delete(id)
    }
    setToasts((current) => current.filter((toast) => toast.id !== id))
  }, [])

  const createToast = useCallback(
    (
      message: string,
      tone: ToastTone,
      options?: { actions?: ToastAction[]; durationMs?: number | null },
    ) => {
      const id = Date.now() + Math.floor(Math.random() * 1000)
      setToasts((current) => [...current, { id, tone, message, actions: options?.actions }])

      const durationMs =
        options?.durationMs === undefined
          ? tone === 'error'
            ? ERROR_TOAST_DURATION_MS
            : DEFAULT_TOAST_DURATION_MS
          : options.durationMs
      if (durationMs !== null) {
        const timeoutId = window.setTimeout(() => {
          dismissToast(id)
        }, durationMs)
        timeoutsRef.current.set(id, timeoutId)
      }
    },
    [dismissToast],
  )

  const notify = useCallback((message: string, tone: ToastTone = 'info') => {
    createToast(message, tone)
  }, [createToast])

  const notifyAction = useCallback(
    (
      message: string,
      options?: { tone?: ToastTone; actions?: ToastAction[]; durationMs?: number | null },
    ) => {
      createToast(message, options?.tone ?? 'info', options)
    },
    [createToast],
  )

  const value = useMemo(() => ({ notify, notifyAction }), [notify, notifyAction])

  return (
    <ToastContext.Provider value={value}>
      {children}
      <div className="toast-stack" aria-live="polite" aria-atomic="true">
        {toasts.map((toast) => (
          <div key={toast.id} className={`toast toast--${toast.tone}`}>
            <p className="toast__message">{toast.message}</p>
            {toast.actions?.length ? (
              <div className="toast__actions">
                {toast.actions.map((action) => (
                  <button
                    key={`${toast.id}:${action.label}`}
                    className={`toast__action toast__action--${action.style ?? 'ghost'}`}
                    type="button"
                    onClick={() => {
                      action.onClick?.()
                      dismissToast(toast.id)
                    }}
                  >
                    {action.label}
                  </button>
                ))}
              </div>
            ) : null}
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  )
}

export function useToast() {
  const context = useContext(ToastContext)
  if (!context) {
    throw new Error('useToast must be used within ToastProvider')
  }

  return context
}
