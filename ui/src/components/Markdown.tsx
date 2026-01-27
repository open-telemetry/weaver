import { marked } from 'marked'
import { useEffect, useState } from 'react'

interface MarkdownProps {
  content: string
}

export function Markdown({ content }: MarkdownProps) {
  const [html, setHtml] = useState('')

  useEffect(() => {
    marked.setOptions({
      breaks: true,
      gfm: true,
    })
    
    const renderedHtml = typeof content === 'string' ? marked(content || '') : ''
    setHtml(renderedHtml)
  }, [content])

  if (!content) return null

  return (
    <div className="prose prose-sm max-w-none" dangerouslySetInnerHTML={{ __html: html }} />
  )
}
