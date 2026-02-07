import { marked } from 'marked'
import { useMemo } from 'react'

interface InlineMarkdownProps {
  content: string
  className?: string
}

export function InlineMarkdown({ content, className = '' }: InlineMarkdownProps) {
  const html = useMemo(() => {
    if (!content || typeof content !== 'string') return ''

    try {
      const result = marked.parse(content, {
        async: false,
        breaks: false,
        gfm: true,
      })

      if (typeof result === 'string') {
        return result.replace(/<\/?p>/g, '')
      }

      return ''
    } catch (e) {
      console.error('Markdown rendering error:', e)
      return content
    }
  }, [content])

  const spanClassName = className ? `inline-markdown ${className}` : 'inline-markdown'

  return (
    <span className={spanClassName} dangerouslySetInnerHTML={{ __html: html }} />
  )
}
