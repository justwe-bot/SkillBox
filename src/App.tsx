import { useEffect, useRef } from 'react'
import { Navigate, Route, Routes } from 'react-router-dom'
import { initializeTheme } from './lib/theme'
import DashboardPage from './pages/DashboardPage'
import SettingsPage from './pages/SettingsPage'
import { ToastProvider, useToast } from './components/ToastProvider'
import { checkUpdates, downloadUpdate, openDownloadedUpdate } from './lib/tauri'

const updateReminderDismissKey = 'skillbox.dismissedUpdateVersion'

function AppShell() {
  const { notify, notifyAction } = useToast()
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
          notifyAction(`发现新版本 ${result.latestVersion}，可以立即下载并安装。`, {
            durationMs: null,
            actions: [
              {
                label: '立即更新',
                style: 'primary',
                onClick: async () => {
                  notify('正在下载更新安装包...', 'info')

                  try {
                    const downloadResult = await downloadUpdate()
                    notify(`更新安装包已下载：${downloadResult.fileName}`, 'success')

                    try {
                      await openDownloadedUpdate(downloadResult.filePath)
                    } catch (error) {
                      notify(`安装包已下载，但自动打开失败: ${String(error)}`, 'error')
                    }
                  } catch (error) {
                    notify(`下载更新失败: ${String(error)}`, 'error')
                  }
                },
              },
              {
                label: '关闭提醒',
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
  }, [notify, notifyAction])

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
