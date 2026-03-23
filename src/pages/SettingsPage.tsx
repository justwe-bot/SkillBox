import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { ArrowLeft, Bell, FolderRoot, Palette } from 'lucide-react'
import { scanApps, getVersion } from '../lib/tauri'
import { useToast } from '../components/ToastProvider'
import type { AppPreferences } from '../types'

const preferenceKey = 'skillbox.preferences'

function loadPreferences(): AppPreferences {
  try {
    const stored = window.localStorage.getItem(preferenceKey)
    if (!stored) {
      return {
        autoScan: true,
        desktopNotifications: true,
        theme: 'system',
      }
    }

    return JSON.parse(stored) as AppPreferences
  } catch {
    return {
      autoScan: true,
      desktopNotifications: true,
      theme: 'system',
    }
  }
}

export default function SettingsPage() {
  const { notify } = useToast()
  const [preferences, setPreferences] = useState<AppPreferences>(loadPreferences)
  const [version, setVersion] = useState('1.0.0')
  const [appPaths, setAppPaths] = useState<string[]>([])

  useEffect(() => {
    void getVersion().then(setVersion).catch(() => undefined)
    void scanApps()
      .then((result) => setAppPaths(result.apps.map((app) => `${app.name}: ${app.path}`)))
      .catch(() => setAppPaths([]))
  }, [])

  function updatePreferences(next: AppPreferences) {
    setPreferences(next)
    window.localStorage.setItem(preferenceKey, JSON.stringify(next))
  }

  function saveSettings() {
    notify('设置已保存到本地。', 'success')
  }

  return (
    <div className="page-shell">
      <header className="hero hero--compact">
        <div>
          <Link className="button button--ghost button--inline" to="/">
            <ArrowLeft size={16} />
            返回总览
          </Link>
          <h1>设置</h1>
          <p className="hero__text">保留 Make 里的设置页结构，但优先适配当前 SkillBox 的实际能力。</p>
        </div>
      </header>

      <main className="settings-grid">
        <section className="surface settings-card">
          <div className="settings-card__header">
            <FolderRoot size={18} />
            <div>
              <h2>扫描路径</h2>
              <p className="muted">这里展示当前后端会去检查的应用技能目录。</p>
            </div>
          </div>
          <div className="stack">
            {appPaths.map((path) => (
              <div className="settings-row" key={path}>
                <span>{path}</span>
              </div>
            ))}
          </div>
        </section>

        <section className="surface settings-card">
          <div className="settings-card__header">
            <Bell size={18} />
            <div>
              <h2>行为偏好</h2>
              <p className="muted">这些选项目前保存在本地，作为 React 版界面的偏好设置。</p>
            </div>
          </div>
          <div className="stack">
            <label className="toggle-row">
              <span>启动时自动扫描</span>
              <input
                type="checkbox"
                checked={preferences.autoScan}
                onChange={(event) => updatePreferences({ ...preferences, autoScan: event.target.checked })}
              />
            </label>
            <label className="toggle-row">
              <span>桌面通知</span>
              <input
                type="checkbox"
                checked={preferences.desktopNotifications}
                onChange={(event) =>
                  updatePreferences({ ...preferences, desktopNotifications: event.target.checked })
                }
              />
            </label>
          </div>
        </section>

        <section className="surface settings-card">
          <div className="settings-card__header">
            <Palette size={18} />
            <div>
              <h2>主题</h2>
              <p className="muted">当前仅记录偏好值，后续可以继续扩展为完整主题切换。</p>
            </div>
          </div>
          <div className="segmented">
            {(['system', 'light', 'dark'] as const).map((theme) => (
              <button
                key={theme}
                className={`segmented__item ${preferences.theme === theme ? 'segmented__item--active' : ''}`}
                type="button"
                onClick={() => updatePreferences({ ...preferences, theme })}
              >
                {theme}
              </button>
            ))}
          </div>
        </section>

        <section className="surface settings-card">
          <div className="settings-card__header">
            <div>
              <h2>关于</h2>
              <p className="muted">React + Tauri rewrite based on your Figma Make project.</p>
            </div>
          </div>
          <div className="stack">
            <div className="settings-row">
              <span>版本</span>
              <strong>{version}</strong>
            </div>
            <div className="settings-row">
              <span>前端框架</span>
              <strong>React</strong>
            </div>
            <div className="settings-row">
              <span>运行壳</span>
              <strong>Tauri</strong>
            </div>
          </div>
          <button className="button button--primary button--full" type="button" onClick={saveSettings}>
            保存设置
          </button>
        </section>
      </main>
    </div>
  )
}
