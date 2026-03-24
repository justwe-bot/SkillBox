interface CodexSkillGlyphProps {
  size?: number
  className?: string
}

export function CodexSkillGlyph({ size = 18, className }: CodexSkillGlyphProps) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <g transform="translate(12 12) scale(1.12) translate(-12.096 -9.984)">
        <path
          d="M12.096 2.4L18.672 6.192V13.776L12.096 17.568L5.52 13.776V6.192L12.096 2.4Z"
          fill="#FFCF5A"
        />
        <path
          d="M12.096 2.4L18.672 6.192L12.096 9.984L5.52 6.192L12.096 2.4Z"
          fill="#FF9643"
        />
        <path
          d="M5.52 6.192L12.096 9.984V17.568L5.52 13.776V6.192Z"
          fill="#7C4DFF"
        />
        <path
          d="M18.672 6.192L12.096 9.984V17.568L18.672 13.776V6.192Z"
          fill="#FFB21C"
        />
        <path
          d="M12.096 9.984L18.672 6.192V13.776L12.096 17.568V9.984Z"
          fill="#E49D18"
          fillOpacity="0.42"
        />
      </g>
    </svg>
  )
}
