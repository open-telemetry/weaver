<script>
  import { marked } from 'marked';

  let { content = '', class: className = '' } = $props();

  // Render markdown to HTML (inline) - simple approach
  function renderMarkdown(text) {
    if (!text || typeof text !== 'string') return '';

    try {
      // Use marked synchronously by accessing the default export directly
      const result = marked.parse(text, {
        async: false,
        breaks: false,
        gfm: true
      });

      // Remove paragraph tags for inline rendering
      if (typeof result === 'string') {
        return result.replace(/<\/?p>/g, '');
      }
      return '';
    } catch (e) {
      console.error('Markdown rendering error:', e);
      return text; // Fallback to plain text
    }
  }

  const html = $derived(renderMarkdown(content));
</script>

<span class={className}>{@html html}</span>

<style>
  /* Inline styles for markdown elements */
  :global(span code) {
    background-color: hsl(var(--b3));
    padding: 0.125rem 0.25rem;
    border-radius: 0.25rem;
    font-size: 0.875em;
  }
  :global(span a) {
    color: hsl(var(--p));
    text-decoration: underline;
  }
  :global(span strong) {
    font-weight: 600;
  }
  :global(span em) {
    font-style: italic;
  }
</style>
