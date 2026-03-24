import { useEffect, useRef, useState } from 'react'

interface AnimatedNumberProps {
  value: number
  className?: string
}

export function AnimatedNumber({ value, className }: AnimatedNumberProps) {
  const [displayValue, setDisplayValue] = useState(value)
  const [isBouncing, setIsBouncing] = useState(false)
  const frameRef = useRef<number | null>(null)
  const bounceTimeoutRef = useRef<number | null>(null)

  useEffect(() => {
    if (frameRef.current) {
      window.cancelAnimationFrame(frameRef.current)
    }

    if (bounceTimeoutRef.current) {
      window.clearTimeout(bounceTimeoutRef.current)
    }

    const startValue = displayValue
    const change = value - startValue

    if (change === 0) {
      return
    }

    const duration = 420
    const startTime = performance.now()
    setIsBouncing(true)
    bounceTimeoutRef.current = window.setTimeout(() => setIsBouncing(false), 520)

    const animate = (now: number) => {
      const progress = Math.min((now - startTime) / duration, 1)
      const eased = 1 - Math.pow(1 - progress, 3)
      setDisplayValue(Math.round(startValue + change * eased))

      if (progress < 1) {
        frameRef.current = window.requestAnimationFrame(animate)
      } else {
        setDisplayValue(value)
      }
    }

    frameRef.current = window.requestAnimationFrame(animate)

    return () => {
      if (frameRef.current) {
        window.cancelAnimationFrame(frameRef.current)
      }
      if (bounceTimeoutRef.current) {
        window.clearTimeout(bounceTimeoutRef.current)
      }
    }
  }, [value])

  return <span className={`${className ?? ''} ${isBouncing ? 'animated-number animated-number--bounce' : ''}`.trim()}>{displayValue}</span>
}
