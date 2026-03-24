import type { AppPreferences } from '../types'
import { loadPreferences, savePreferences } from './preferences'

type ThemeMode = AppPreferences['theme']
type AppliedTheme = 'light' | 'dark'

function resolveSystemTheme(): AppliedTheme {
  if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
    return 'dark'
  }

  return 'light'
}

export function resolveAppliedTheme(theme: ThemeMode): AppliedTheme {
  return theme === 'system' ? resolveSystemTheme() : theme
}

export function applyTheme(theme: ThemeMode) {
  const applied = resolveAppliedTheme(theme)
  const root = document.documentElement
  root.dataset.theme = applied
  root.style.colorScheme = applied
}

export function initializeTheme() {
  applyTheme(loadPreferences().theme)
}

export function updateThemePreference(theme: ThemeMode) {
  const preferences = loadPreferences()
  const next = { ...preferences, theme }
  savePreferences(next)
  applyTheme(theme)
  return next
}

export function toggleThemePreference() {
  const current = loadPreferences().theme
  const next: ThemeMode = resolveAppliedTheme(current) === 'dark' ? 'light' : 'dark'
  return updateThemePreference(next)
}
