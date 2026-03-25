import type { AppPreferences } from '../types'
import { resolveInitialLanguage } from './i18n'

export const preferenceKey = 'skillbox.preferences'

export const defaultPreferences: AppPreferences = {
  autoScan: true,
  autoSync: false,
  desktopNotifications: true,
  theme: 'system',
  language: 'zh-CN',
}

export function loadPreferences(): AppPreferences {
  try {
    const stored = window.localStorage.getItem(preferenceKey)
    if (!stored) {
      const next = { ...defaultPreferences, language: resolveInitialLanguage() }
      savePreferences(next)
      return next
    }

    const parsed = JSON.parse(stored) as Partial<AppPreferences>
    const next = { ...defaultPreferences, ...parsed }

    if (!parsed.language) {
      next.language = resolveInitialLanguage()
      savePreferences(next)
    }

    return next
  } catch {
    return defaultPreferences
  }
}

export function savePreferences(next: AppPreferences) {
  window.localStorage.setItem(preferenceKey, JSON.stringify(next))
}
