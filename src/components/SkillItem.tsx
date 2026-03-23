import { AlertCircle, ExternalLink, FileCode2 } from 'lucide-react'
import type { SkillRecord } from '../types'

interface SkillItemProps {
  skill: SkillRecord
  onOpen: (path: string) => void
}

function formatSize(size: number) {
  if (size < 1024) {
    return `${size} B`
  }

  return `${(size / 1024).toFixed(1)} KB`
}

export function SkillItem({ skill, onOpen }: SkillItemProps) {
  return (
    <article className={`surface skill-item ${skill.conflicts ? 'skill-item--conflict' : ''}`}>
      <div className="skill-item__meta">
        <div className="skill-item__icon">
          <FileCode2 size={18} />
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
      <button className="button button--ghost" type="button" onClick={() => onOpen(skill.path)}>
        <ExternalLink size={16} />
        打开
      </button>
    </article>
  )
}
