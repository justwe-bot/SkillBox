import { useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { open as openDialog } from '@tauri-apps/api/dialog'
import { open as openPath } from '@tauri-apps/api/shell'
import { FolderPlus, Scan, Search, Settings } from 'lucide-react'
import { ApplicationCard } from '../components/ApplicationCard'
import { FigmaSkillIcon } from '../components/FigmaSkillIcon'
import { GitPanel } from '../components/GitPanel'
import { Modal } from '../components/Modal'
import { SkillItem } from '../components/SkillItem'
import { useToast } from '../components/ToastProvider'
import {
  addCustomApp,
  linkApp,
  loadSkillInventory,
  saveGitPath,
  scanApps,
  setCustomPath,
  syncToGit,
  unlinkApp,
} from '../lib/tauri'
import type { AppRecord, SkillRecord } from '../types'

type TabKey = 'applications' | 'skills'

export default function DashboardPage() {
  const { notify } = useToast()
  const [activeTab, setActiveTab] = useState<TabKey>('applications')
  const [apps, setApps] = useState<AppRecord[]>([])
  const [skills, setSkills] = useState<SkillRecord[]>([])
  const [gitPath, setGitPath] = useState('')
  const [loading, setLoading] = useState(true)
  const [busyAppId, setBusyAppId] = useState<string | null>(null)
  const [syncing, setSyncing] = useState(false)
  const [search, setSearch] = useState('')
  const [customModalOpen, setCustomModalOpen] = useState(false)
  const [editModalOpen, setEditModalOpen] = useState(false)
  const [editingApp, setEditingApp] = useState<AppRecord | null>(null)
  const [customAppName, setCustomAppName] = useState('')
  const [customAppPath, setCustomAppPath] = useState('')
  const [editPathValue, setEditPathValue] = useState('')

  async function refreshData(showToast = false) {
    setLoading(true)

    try {
      const appState = await scanApps()
      setApps(appState.apps)
      setGitPath(appState.gitPath)
      setSkills(await loadSkillInventory(appState.apps))

      if (showToast) {
        notify(`扫描完成，共发现 ${appState.apps.length} 个应用`, 'success')
      }
    } catch (error) {
      notify(`加载失败: ${String(error)}`, 'error')
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void refreshData()
  }, [])

  const filteredSkills = useMemo(() => {
    if (!search.trim()) {
      return skills
    }

    const query = search.trim().toLowerCase()
    return skills.filter((skill) => {
      return (
        skill.name.toLowerCase().includes(query) ||
        skill.description.toLowerCase().includes(query) ||
        skill.sources.some((source) => source.toLowerCase().includes(query))
      )
    })
  }, [search, skills])

  const stats = useMemo(() => {
    return {
      appCount: apps.length,
      skillCount: skills.length,
      linkedCount: apps.filter((app) => app.isLinked).length,
      conflictCount: skills.filter((skill) => skill.conflicts).length,
    }
  }, [apps, skills])

  async function handleToggleLink(app: AppRecord) {
    if (!app.isLinked && !gitPath) {
      notify('请先配置 Git 同步目录，再执行链接。', 'error')
      return
    }

    setBusyAppId(app.id)

    try {
      if (app.isLinked) {
        await unlinkApp(app.id)
        notify(`已取消 ${app.name} 的软链接`, 'success')
      } else {
        await linkApp(app.id, gitPath)
        notify(`已创建 ${app.name} 的软链接`, 'success')
      }

      await refreshData()
    } catch (error) {
      notify(`操作失败: ${String(error)}`, 'error')
    } finally {
      setBusyAppId(null)
    }
  }

  async function handlePickGitFolder() {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: '选择技能同步目录',
    })

    if (typeof selected !== 'string' || !selected) {
      return
    }

    try {
      await saveGitPath(selected)
      setGitPath(selected)
      notify('同步目录已保存', 'success')
    } catch (error) {
      notify(`保存目录失败: ${String(error)}`, 'error')
    }
  }

  async function handleSync() {
    if (!gitPath) {
      notify('请先选择同步目录', 'error')
      return
    }

    setSyncing(true)
    try {
      await syncToGit(gitPath)
      await refreshData()
      notify('已同步当前检测到的技能到仓库目录', 'success')
    } catch (error) {
      notify(`同步失败: ${String(error)}`, 'error')
    } finally {
      setSyncing(false)
    }
  }

  function handleSaveGitConfig(_config: { repoUrl: string; username: string; branch: string }) {
    notify('界面配置已保存，同步目录仍然使用当前 Tauri 配置。', 'success')
  }

  async function chooseCustomPath(setter: (path: string) => void) {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: '选择路径',
    })

    if (typeof selected === 'string' && selected) {
      setter(selected)
    }
  }

  async function handleAddCustomApp() {
    if (!customAppName.trim() || !customAppPath.trim()) {
      notify('请填写应用名称和路径', 'error')
      return
    }

    try {
      await addCustomApp(customAppName.trim(), customAppPath.trim())
      setCustomAppName('')
      setCustomAppPath('')
      setCustomModalOpen(false)
      await refreshData()
      notify('自定义应用已添加', 'success')
    } catch (error) {
      notify(`添加失败: ${String(error)}`, 'error')
    }
  }

  function openEditPath(app: AppRecord) {
    setEditingApp(app)
    setEditPathValue(app.customPath ?? app.path)
    setEditModalOpen(true)
  }

  async function handleSavePath() {
    if (!editingApp) {
      return
    }

    try {
      const customPath = editPathValue.trim() ? editPathValue.trim() : null
      await setCustomPath(editingApp.id, customPath)
      setEditModalOpen(false)
      setEditingApp(null)
      await refreshData()
      notify('路径已更新', 'success')
    } catch (error) {
      notify(`更新路径失败: ${String(error)}`, 'error')
    }
  }

  async function handleOpenSkill(path: string) {
    try {
      await openPath(path)
    } catch (error) {
      notify(`无法打开路径: ${String(error)}`, 'error')
    }
  }

  return (
    <div className="page-shell">
      <header className="hero">
        <div className="hero__brand">
          <FigmaSkillIcon size={74} />
          <div>
            <h1>AI Skills Manager</h1>
            <p className="hero__text">统一管理所有 AI 应用的技能</p>
          </div>
        </div>
        <div className="hero__actions">
          <button className="button button--primary button--hero" type="button" onClick={() => void refreshData(true)} disabled={loading}>
            <Scan size={18} />
            {loading ? '扫描中...' : '扫描应用'}
          </button>
          <Link className="button button--square" to="/settings" aria-label="设置">
            <Settings size={22} />
          </Link>
        </div>
      </header>

      <section className="stats-grid">
        <article className="surface stat-card">
          <span className="stat-card__label">检测到的应用</span>
          <strong>{stats.appCount}</strong>
        </article>
        <article className="surface stat-card">
          <span className="stat-card__label">技能文件总数</span>
          <strong>{stats.skillCount}</strong>
        </article>
        <article className="surface stat-card">
          <span className="stat-card__label">已链接应用</span>
          <strong>{stats.linkedCount}</strong>
        </article>
        <article className="surface stat-card">
          <span className="stat-card__label">冲突条目</span>
          <strong className={stats.conflictCount ? 'danger' : ''}>{stats.conflictCount}</strong>
        </article>
      </section>

      <main className="dashboard-grid">
        <section className="main-column">
          {activeTab === 'applications' ? (
            <section className="stack">
              <div className="section-toolbar section-toolbar--applications">
                <div className="tabs tabs--embedded">
                  <button className="tab tab--active" type="button" onClick={() => setActiveTab('applications')}>
                    应用列表
                  </button>
                  <button className="tab" type="button" onClick={() => setActiveTab('skills')}>
                    技能管理
                    {stats.conflictCount ? <span className="tab__count tab__count--danger">{stats.conflictCount}</span> : null}
                  </button>
                </div>
                <button className="button button--ghost" type="button" onClick={() => setCustomModalOpen(true)}>
                  <FolderPlus size={16} />
                  添加自定义应用
                </button>
              </div>

              {apps.length === 0 ? (
                <div className="surface empty-state">
                  <p>当前没有发现任何应用，点击上方“重新扫描”后再试。</p>
                </div>
              ) : (
                apps.map((app) => (
                  <ApplicationCard
                    key={app.id}
                    app={app}
                    busy={busyAppId === app.id}
                    onToggleLink={handleToggleLink}
                    onEditPath={openEditPath}
                  />
                ))
              )}
            </section>
          ) : (
            <section className="stack">
              <div className="section-toolbar section-toolbar--align-start section-toolbar--applications">
                <div className="tabs tabs--embedded">
                  <button className="tab" type="button" onClick={() => setActiveTab('applications')}>
                    应用列表
                  </button>
                  <button className="tab tab--active" type="button">
                    技能管理
                    {stats.conflictCount ? <span className="tab__count">{stats.conflictCount}</span> : null}
                  </button>
                </div>
                <label className="search-box">
                  <Search size={16} />
                  <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索技能名称、路径或来源" />
                </label>
              </div>

              {filteredSkills.length === 0 ? (
                <div className="surface empty-state">
                  <p>{search ? '没有匹配的技能结果。' : '当前没有可显示的技能文件。'}</p>
                </div>
              ) : (
                filteredSkills.map((skill) => <SkillItem key={skill.id} skill={skill} onOpen={handleOpenSkill} />)
              )}
            </section>
          )}
        </section>

        <aside className="side-column">
          <GitPanel gitPath={gitPath} syncing={syncing} onSaveConfig={handleSaveGitConfig} onSync={handleSync} />
          <button className="button button--ghost button--full side-column__picker" type="button" onClick={() => void handlePickGitFolder()}>
            选择本地同步目录
          </button>
        </aside>
      </main>

      <Modal open={customModalOpen} title="添加自定义应用" onClose={() => setCustomModalOpen(false)}>
        <div className="modal__body">
          <div className="field-group">
            <label htmlFor="custom-app-name">应用名称</label>
            <input
              id="custom-app-name"
              value={customAppName}
              onChange={(event) => setCustomAppName(event.target.value)}
              placeholder="例如 My Agent"
            />
          </div>
          <div className="field-group">
            <label htmlFor="custom-app-path">技能目录</label>
            <div className="inline-field">
              <input
                id="custom-app-path"
                value={customAppPath}
                onChange={(event) => setCustomAppPath(event.target.value)}
                placeholder="/path/to/skills"
              />
              <button className="button button--ghost" type="button" onClick={() => void chooseCustomPath(setCustomAppPath)}>
                浏览
              </button>
            </div>
          </div>
        </div>
        <footer className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => setCustomModalOpen(false)}>
            取消
          </button>
          <button className="button button--primary" type="button" onClick={() => void handleAddCustomApp()}>
            添加
          </button>
        </footer>
      </Modal>

      <Modal open={editModalOpen} title={editingApp ? `设置 ${editingApp.name} 路径` : '设置路径'} onClose={() => setEditModalOpen(false)}>
        <div className="modal__body">
          <div className="field-group">
            <label htmlFor="edit-app-path">技能目录</label>
            <div className="inline-field">
              <input
                id="edit-app-path"
                value={editPathValue}
                onChange={(event) => setEditPathValue(event.target.value)}
                placeholder="清空则恢复默认路径"
              />
              <button className="button button--ghost" type="button" onClick={() => void chooseCustomPath(setEditPathValue)}>
                浏览
              </button>
            </div>
          </div>
        </div>
        <footer className="modal__footer">
          <button className="button button--ghost" type="button" onClick={() => setEditPathValue('')}>
            恢复默认
          </button>
          <button className="button button--primary" type="button" onClick={() => void handleSavePath()}>
            保存
          </button>
        </footer>
      </Modal>
    </div>
  )
}
