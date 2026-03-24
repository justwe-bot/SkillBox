import { useState } from 'react'
import { AlertCircle, Eye, MoreVertical, PencilLine, Trash2 } from 'lucide-react'
import type { SkillRecord } from '../types'
import { CodexSkillGlyph } from './CodexSkillGlyph'

interface SkillItemProps {
  skill: SkillRecord
  onView: (skill: SkillRecord) => void
  onRename: (skill: SkillRecord) => void
  onDelete: (skill: SkillRecord) => void
  onResolveConflict: (skill: SkillRecord) => void
}

function formatSize(size: number) {
  if (size < 1024) {
    return `${size} B`
  }

  return `${(size / 1024).toFixed(1)} KB`
}

export function SkillItem({ skill, onView, onRename, onDelete, onResolveConflict }: SkillItemProps) {
  const [menuOpen, setMenuOpen] = useState(false)

  function handleAction(action: () => void) {
    setMenuOpen(false)
    action()
  }

  return (
    <article className={`surface skill-item ${skill.conflicts ? 'skill-item--conflict' : ''}`}>
      <div className="skill-item__meta">
        <div className="skill-item__icon">
          <CodexSkillGlyph size={28} />
        </div>
        <div className="skill-item__content">
          <div className="skill-item__title-row">
            <h4>{skill.name}</h4>
            {skill.conflicts ? (
              <span className="badge badge--danger">
                <AlertCircle size={12} />
                冲突
              </span>
            ) : null}
          </div>
          <p className="muted ellipsis">{skill.description}</p>
          <div className="skill-item__tags">
            <span>来源: {skill.sources.join(', ')}</span>
            <span>{formatSize(skill.size)}</span>
            <span>{skill.modified}</span>
          </div>
        </div>
      </div>

      <div className="skill-item__actions">
        {skill.conflicts ? (
          <button className="button button--ghost button--compact" type="button" onClick={() => onResolveConflict(skill)}>
            解决冲突
          </button>
        ) : null}

        <div className="skill-item__menu">
          <button
            className="button button--icon-ghost"
            type="button"
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            onClick={() => setMenuOpen((current) => !current)}
          >
            <MoreVertical size={16} />
          </button>

          {menuOpen ? (
            <div className="skill-item__menu-list" role="menu">
              <button className="skill-item__menu-item" type="button" role="menuitem" onClick={() => handleAction(() => onView(skill))}>
                <Eye size={15} />
                查看详情
              </button>
              <button className="skill-item__menu-item" type="button" role="menuitem" onClick={() => handleAction(() => onRename(skill))}>
                <PencilLine size={15} />
                重命名
              </button>
              <button
                className="skill-item__menu-item skill-item__menu-item--danger"
                type="button"
                role="menuitem"
                onClick={() => handleAction(() => onDelete(skill))}
              >
                <Trash2 size={15} />
                删除
              </button>
            </div>
          ) : null}
        </div>
      </div>
    </article>
  )
}
