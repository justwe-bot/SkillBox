import { useEffect, useRef } from 'react'
import { Navigate, Route, Routes } from 'react-router-dom'
import { useI18n } from './lib/i18n-context'
import { initializeTheme } from './lib/theme'
import DashboardPage from './pages/DashboardPage'
import SettingsPage from './pages/SettingsPage'
import { ToastProvider, useToast } from './components/ToastProvider'
import { checkUpdates, downloadUpdate, openDownloadedUpdate } from './lib/tauri'

const updateReminderDismissKey = 'skillbox.dismissedUpdateVersion'

function AppShell() {
  const { notify, notifyAction } = useToast()
  const { t } = useI18n()
  const remindedVersionRef = useRef<string | null>(null)

  useEffect(() => {
    initializeTheme()
  }, [])

  useEffect(() => {
    const timerId = window.setTimeout(() => {
      void (async () => {
        try {
          const result = await checkUpdates()
          if (!result.updateAvailable || !result.latestVersion) {
            return
          }

          const dismissedVersion = window.localStorage.getItem(updateReminderDismissKey)
          if (dismissedVersion === result.latestVersion || remindedVersionRef.current === result.latestVersion) {
            return
          }

          remindedVersionRef.current = result.latestVersion
          notifyAction(t('app.updateAvailable', { version: result.latestVersion }), {
            durationMs: null,
            actions: [
              {
                label: t('app.updateNow'),
                style: 'primary',
                onClick: async () => {
                  notify(t('app.downloadingInstaller'), 'info')

                  try {
                    const downloadResult = await downloadUpdate()
                    notify(t('app.installerDownloaded', { fileName: downloadResult.fileName }), 'success')

                    try {
                      await openDownloadedUpdate(downloadResult.filePath)
                    } catch (error) {
                      notify(t('app.installerOpenFailed', { error: String(error) }), 'error')
                    }
                  } catch (error) {
                    notify(t('app.downloadFailed', { error: String(error) }), 'error')
                  }
                },
              },
              {
                label: t('app.dismissReminder'),
                onClick: () => {
                  window.localStorage.setItem(updateReminderDismissKey, result.latestVersion ?? '')
                },
              },
            ],
          })
        } catch {
          // Background update checks should stay silent to avoid interrupting the user.
        }
      })()
    }, 30000)

    return () => {
      window.clearTimeout(timerId)
    }
  }, [notify, notifyAction, t])

  return (
    <Routes>
      <Route path="/" element={<DashboardPage />} />
      <Route path="/settings" element={<SettingsPage />} />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  )
}

function App() {
  return (
    <ToastProvider>
      <AppShell />
    </ToastProvider>
  )
}

export default App
