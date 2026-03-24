import {
  Sparkles,
  Wrench,
} from 'lucide-react'
import {
  siClaude,
  siCline,
  siCursor,
  siGithubcopilot,
  siGooglegemini,
  siWindsurf,
  type SimpleIcon,
} from 'simple-icons'

interface AppBrandIconProps {
  appId: string
  appName: string
}

function BrandSvg({
  icon,
  label,
  background,
}: {
  icon: SimpleIcon
  label: string
  background?: string
}) {
  return (
    <span className="brand-icon" style={{ background: background ?? '#f6f7fb', color: `#${icon.hex}` }} aria-hidden="true">
      <svg viewBox="0 0 24 24" role="img" aria-label={label}>
        <path d={icon.path} fill="currentColor" />
      </svg>
    </span>
  )
}

function BrandImageIcon({
  src,
  label,
  background,
  padded = false,
  imageClassName,
}: {
  src: string
  label: string
  background?: string
  padded?: boolean
  imageClassName?: string
}) {
  return (
    <span className="brand-icon" style={{ background: background ?? '#f6f7fb' }} aria-hidden="true">
      <img
        className={['brand-icon__image', padded ? 'brand-icon__image--padded' : '', imageClassName ?? ''].filter(Boolean).join(' ')}
        src={src}
        alt={label}
      />
    </span>
  )
}

function MonogramIcon({ appName }: { appName: string }) {
  const monogram = appName
    .split(/\s+/)
    .map((part) => part[0] ?? '')
    .join('')
    .slice(0, 2)
    .toUpperCase()

  return (
    <span className="brand-icon brand-icon--monogram" aria-hidden="true">
      {monogram || 'AI'}
    </span>
  )
}

export function AppBrandIcon({ appId, appName }: AppBrandIconProps) {
  switch (appId) {
    case 'codex':
      return (
        <BrandImageIcon
          src="/brand-icons/codex.png"
          label="Codex"
          background="#ffffff"
          padded
          imageClassName="brand-icon__image--codex"
        />
      )
    case 'cursor':
      return <BrandSvg icon={siCursor} label="Cursor" background="#f3f5fb" />
    case 'windsurf':
      return <BrandSvg icon={siWindsurf} label="Windsurf" background="#eef7ff" />
    case 'copilot':
      return <BrandSvg icon={siGithubcopilot} label="GitHub Copilot" background="#f4f8ff" />
    case 'claude':
      return <BrandSvg icon={siClaude} label="Claude" background="#fbf5ef" />
    case 'cline':
      return <BrandSvg icon={siCline} label="Cline" background="#f4f5ff" />
    case 'gemini':
      return <BrandSvg icon={siGooglegemini} label="Gemini CLI" background="#f5f7ff" />
    case 'trae':
      return <BrandImageIcon src="/brand-icons/trae.png" label="Trae" background="#ffffff" padded />
    case 'kiro':
      return <BrandImageIcon src="/brand-icons/kiro.svg" label="Kiro" background="#ffffff" padded imageClassName="brand-icon__image--kiro" />
    case 'qoder':
      return <BrandImageIcon src="/brand-icons/qoder.svg" label="Qoder" background="#ffffff" padded imageClassName="brand-icon__image--qoder" />
    case 'codebuddy':
      return <BrandImageIcon src="/brand-icons/codebuddy.png" label="CodeBuddy" background="#ffffff" padded imageClassName="brand-icon__image--codebuddy" />
    case 'continue':
      return <BrandImageIcon src="/brand-icons/continue.png" label="Continue" background="#ffffff" padded />
    case 'aider':
      return (
        <span className="brand-icon brand-icon--accent brand-icon--aider" aria-hidden="true">
          <Wrench size={24} strokeWidth={2.2} />
        </span>
      )
    case 'opencode':
      return (
        <BrandImageIcon
          src="/brand-icons/opencode.png"
          label="OpenCode"
          background="#ffffff"
          padded
          imageClassName="brand-icon__image--opencode"
        />
      )
    case 'openclaw':
      return <BrandImageIcon src="/brand-icons/openclaw.svg" label="OpenClaw" background="#fff7ef" padded />
    case 'custom':
      return <MonogramIcon appName={appName} />
    default:
      if (appId.startsWith('custom-')) {
        return <MonogramIcon appName={appName} />
      }

      return (
        <span className="brand-icon brand-icon--accent brand-icon--fallback" aria-hidden="true">
          <Sparkles size={22} strokeWidth={2.2} />
        </span>
      )
  }
}
