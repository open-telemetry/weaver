import { marked } from 'marked'
import { useEffect, useState } from 'react'

interface InlineMarkdownProps {
  content: string
  className?: string
}

export function InlineMarkdown({ content, className = '' }: InlineMarkdownProps) {
  const [html, setHtml] = useState('')

  useEffect(() => {
    // Render markdown to HTML (inline) - simple approach
    const renderInlineMarkdown = (text: string) => {
      if (!text || typeof text !== 'string') return ''

      try {
        // Use marked synchronously by accessing the default export directly
        const result = marked.parse(text, {
          async: false,
          breaks: false,
          gfm: true,
        })

        // Strip paragraph tags for inline rendering (exact same as Svelte)
        if (typeof result === 'string') {
          return result.replace(/<\/?p>/g, '')
        }

        return ''
      } catch (e) {
        console.error('Markdown rendering error:', e)
        return text // Fallback to plain text
      }
    }

    // Set the rendered HTML
    setHtml(renderInlineMarkdown(content))
  }, [content])

  const spanClassName = className ? `inline-markdown ${className}` : 'inline-markdown'

  return (
    <span className={spanClassName} dangerouslySetInnerHTML={{ __html: html }} />
  )
}

