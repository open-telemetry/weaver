import { marked } from 'marked'
import { useMemo } from 'react'
import DOMPurify from 'dompurify'

interface MarkdownProps {
  content: string
}

export function Markdown({ content }: MarkdownProps) {
  const html = useMemo(() => {
    marked.setOptions({
      breaks: true,
      gfm: true,
    })

    if (typeof content !== 'string') return ''

    const rendered = marked(content || '')
    return DOMPurify.sanitize(rendered)
  }, [content])

  if (!content) return null

  return (
    <div className="prose prose-sm max-w-none" dangerouslySetInnerHTML={{ __html: html }} />
  )
}
