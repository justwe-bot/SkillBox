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
      <path
        d="M39 24H67L82 39V85H39C34.5817 85 31 81.4183 31 77V32C31 27.5817 34.5817 24 39 24Z"
        stroke="white"
        strokeWidth="7"
        strokeLinejoin="round"
      />
      <path
        d="M67 24V39H82"
        stroke="white"
        strokeWidth="7"
        strokeLinejoin="round"
      />
      <path
        d="M46 56L39 63L46 70"
        stroke="white"
        strokeWidth="8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M66 56L73 63L66 70"
        stroke="white"
        strokeWidth="8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M57 53L54 73"
        stroke="white"
        strokeWidth="8"
        strokeLinecap="round"
      />
    </svg>
  )
}
