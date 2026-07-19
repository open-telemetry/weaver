import { useEffect, useRef, useState } from 'react'

const copiedResetMs = 2000

export function useCopyToClipboard() {
  const [copied, setCopied] = useState(false)
  const timeoutRef = useRef<number | null>(null)

  useEffect(() => {
    return () => {
      if (timeoutRef.current !== null) {
        window.clearTimeout(timeoutRef.current)
      }
    }
  }, [])

  async function copyToClipboard(text: string) {
    await navigator.clipboard.writeText(text)
    setCopied(true)

    if (timeoutRef.current !== null) {
      window.clearTimeout(timeoutRef.current)
    }

    timeoutRef.current = window.setTimeout(() => {
      setCopied(false)
    }, copiedResetMs)
  }

  return { copied, copyToClipboard }
}
