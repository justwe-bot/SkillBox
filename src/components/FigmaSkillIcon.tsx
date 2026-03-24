interface FigmaSkillIconProps {
  size?: number
  className?: string
}

export function FigmaSkillIcon({ size = 74, className }: FigmaSkillIconProps) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 112 112"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <rect width="112" height="112" rx="30" fill="#06061A" />
      <g transform="translate(56, 56) scale(3.95) translate(-12, -12)" fill="#FFFFFF">
        <path d="M 4 13 L 8 11 L 12 13 L 8 15 Z" opacity="0.8" />
        <path d="M 4 13 L 4 17 L 8 19 L 8 15 Z" opacity="0.5" />
        <path d="M 8 15 L 8 19 L 12 17 L 12 13 Z" opacity="0.6" />

        <path d="M 12 13 L 16 11 L 20 13 L 16 15 Z" opacity="0.8" />
        <path d="M 12 13 L 12 17 L 16 19 L 16 15 Z" opacity="0.5" />
        <path d="M 16 15 L 16 19 L 20 17 L 20 13 Z" opacity="0.6" />

        <path d="M 8 7 L 12 5 L 16 7 L 12 9 Z" opacity="0.9" />
        <path d="M 8 7 L 8 11 L 12 13 L 12 9 Z" opacity="0.6" />
        <path d="M 12 9 L 12 13 L 16 11 L 16 7 Z" opacity="0.7" />

        <path d="M 8 11 L 12 13 L 16 11" stroke="#FFFFFF" strokeWidth="0.5" opacity="0.4" />
      </g>
    </svg>
  )
}
