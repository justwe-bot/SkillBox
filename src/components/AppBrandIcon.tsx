import { Sparkles } from 'lucide-react'

interface AppBrandIconProps {
  appId: string
  appName: string
}

function BrandImageIcon({
  src,
  label,
}: {
  src: string
  label: string
}) {
  return (
    <span className="brand-icon" aria-hidden="true">
      <img className="brand-icon__image" src={src} alt={label} />
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

const BRAND_ICON_PATHS: Record<string, string> = {
  claude: '/brand-icons/unified/claude.png',
  cline: '/brand-icons/unified/cline.png',
  codebuddy: '/brand-icons/unified/codebuddy.png',
  codex: '/brand-icons/unified/codex.png',
  copilot: '/brand-icons/unified/copilot.png',
  cursor: '/brand-icons/unified/cursor.png',
  gemini: '/brand-icons/unified/gemini.png',
  kiro: '/brand-icons/unified/kiro.png',
  openclaw: '/brand-icons/unified/openclaw.png',
  opencode: '/brand-icons/unified/opencode.png',
  qoder: '/brand-icons/unified/qoder.png',
  roocode: '/brand-icons/unified/roocode.png',
  trae: '/brand-icons/unified/trae.png',
  windsurf: '/brand-icons/unified/windsurf.png',
}

export function AppBrandIcon({ appId, appName }: AppBrandIconProps) {
  const iconPath = BRAND_ICON_PATHS[appId]

  if (iconPath) {
    return <BrandImageIcon src={iconPath} label={appName} />
  }

  switch (appId) {
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
