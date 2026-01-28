import { marked } from 'marked'
import { useEffect, useState } from 'react'

interface InlineMarkdownProps {
  content: string
  className?: string
}

export function InlineMarkdown({ content, className = '' }: InlineMarkdownProps) {
  const [html, setHtml] = useState('')

  useEffect(() => {
    const renderInlineMarkdown = (text: string) => {
      if (!text || typeof text !== 'string') return ''

      try {
        const result = marked.parse(text, {
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
        return text
      }
    }

    setHtml(renderInlineMarkdown(content))
  }, [content])

  const spanClassName = className ? `inline-markdown ${className}` : 'inline-markdown'

  return (
    <span className={spanClassName} dangerouslySetInnerHTML={{ __html: html }} />
  )
}
