import { marked } from 'marked'
import { useEffect, useState } from 'react'

interface MarkdownProps {
  content: string
}

export function Markdown({ content }: MarkdownProps) {
  const [html, setHtml] = useState('')

  useEffect(() => {
    // Configure marked for safe rendering (same as Svelte)
    marked.setOptions({
      breaks: true,
      gfm: true,
    })
    
    // Render markdown to HTML synchronously
    const renderedHtml = typeof content === 'string' ? marked(content || '') : ''
    setHtml(renderedHtml)
  }, [content])

  if (!content) return null

  return (
    <div className="prose prose-sm max-w-none" dangerouslySetInnerHTML={{ __html: html }} />
  )
}