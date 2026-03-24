import { Moon, Sun } from 'lucide-react'
import { useEffect, useState } from 'react'
import { loadPreferences } from '../lib/preferences'
import { resolveAppliedTheme, toggleThemePreference } from '../lib/theme'

export function ThemeToggle() {
  const [theme, setTheme] = useState<'light' | 'dark'>(() => resolveAppliedTheme(loadPreferences().theme))

  useEffect(() => {
    setTheme(resolveAppliedTheme(loadPreferences().theme))
  }, [])

  function handleToggle() {
    const next = toggleThemePreference()
    setTheme(resolveAppliedTheme(next.theme))
  }

  return (
    <button className="button button--square button--theme" type="button" onClick={handleToggle} aria-label="切换主题">
      {theme === 'light' ? <Moon size={18} /> : <Sun size={18} />}
    </button>
  )
}
