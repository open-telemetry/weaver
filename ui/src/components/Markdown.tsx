import { marked } from 'marked'
import { useMemo } from 'react'

interface MarkdownProps {
  content: string
}

export function Markdown({ content }: MarkdownProps) {
  const html = useMemo(() => {
    marked.setOptions({
      breaks: true,
      gfm: true,
    })

    return typeof content === 'string' ? marked(content || '') : ''
  }, [content])

  if (!content) return null

  return (
    <div className="prose prose-sm max-w-none" dangerouslySetInnerHTML={{ __html: html }} />
  )
}
