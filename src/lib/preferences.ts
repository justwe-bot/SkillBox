import type { AppPreferences } from '../types'

export const preferenceKey = 'skillbox.preferences'

export const defaultPreferences: AppPreferences = {
  autoScan: true,
  autoSync: false,
  desktopNotifications: true,
  theme: 'system',
}

export function loadPreferences(): AppPreferences {
  try {
    const stored = window.localStorage.getItem(preferenceKey)
    if (!stored) {
      return defaultPreferences
    }

    return { ...defaultPreferences, ...(JSON.parse(stored) as Partial<AppPreferences>) }
  } catch {
    return defaultPreferences
  }
}

export function savePreferences(next: AppPreferences) {
  window.localStorage.setItem(preferenceKey, JSON.stringify(next))
}
