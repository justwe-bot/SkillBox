import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type PropsWithChildren,
} from 'react'
import type { AppLanguage } from '../types'
import { getTranslator, type TranslationKey, type TranslationParams } from './i18n'
import { loadPreferences } from './preferences'

interface I18nContextValue {
  language: AppLanguage
  setLanguage: (language: AppLanguage) => void
  t: (key: TranslationKey, params?: TranslationParams) => string
}

const I18nContext = createContext<I18nContextValue | null>(null)

export function LanguageProvider({ children }: PropsWithChildren) {
  const [language, setLanguage] = useState<AppLanguage>(() => loadPreferences().language)

  useEffect(() => {
    document.documentElement.lang = language
  }, [language])

  const value = useMemo<I18nContextValue>(() => ({
    language,
    setLanguage,
    t: getTranslator(language),
  }), [language])

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}

export function useI18n() {
  const context = useContext(I18nContext)
  if (!context) {
    throw new Error('useI18n must be used within LanguageProvider')
  }

  return context
}
