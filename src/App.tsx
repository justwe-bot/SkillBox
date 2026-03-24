import { useEffect } from 'react'
import { Navigate, Route, Routes } from 'react-router-dom'
import { initializeTheme } from './lib/theme'
import DashboardPage from './pages/DashboardPage'
import SettingsPage from './pages/SettingsPage'
import { ToastProvider } from './components/ToastProvider'

function App() {
  useEffect(() => {
    initializeTheme()
  }, [])

  return (
    <ToastProvider>
      <Routes>
        <Route path="/" element={<DashboardPage />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </ToastProvider>
  )
}

export default App
