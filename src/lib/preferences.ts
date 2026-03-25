import type { AppPreferences } from '../types'
import { resolveInitialLanguage } from './i18n'

export const preferenceKey = 'skillbox.preferences'

export const defaultPreferences: AppPreferences = {
  autoScan: true,
  autoSync: false,
  desktopNotifications: true,
  theme: 'system',
  language: 'zh-CN',
  onboardingCompleted: false,
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
    }

    // Existing users may have preferences stored before onboarding was added.
    // Treat them as already onboarded so only true first-time installs see the guide.
    if (!Object.prototype.hasOwnProperty.call(parsed, 'onboardingCompleted')) {
      next.onboardingCompleted = true
    }

    if (!parsed.language || !Object.prototype.hasOwnProperty.call(parsed, 'onboardingCompleted')) {
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
