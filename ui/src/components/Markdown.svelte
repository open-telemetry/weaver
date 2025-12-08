<script>
  import { marked } from 'marked';

  let { content = '' } = $props();

  // Configure marked for safe rendering
  marked.setOptions({
    breaks: true,
    gfm: true,
  });

  // Render markdown to HTML synchronously
  const html = $derived(typeof content === 'string' ? marked(content || '') : '');
</script>

<div class="prose prose-sm max-w-none">{@html html}</div>

<style>
  /* Style the rendered markdown */
  :global(.prose) {
    color: inherit;
  }
  :global(.prose a) {
    color: hsl(var(--p));
    text-decoration: underline;
  }
  :global(.prose code) {
    background-color: hsl(var(--b3));
    padding: 0.125rem 0.25rem;
    border-radius: 0.25rem;
    font-size: 0.875em;
  }
  :global(.prose pre) {
    background-color: hsl(var(--b3));
    padding: 1rem;
    border-radius: 0.5rem;
    overflow-x: auto;
  }
  :global(.prose pre code) {
    background-color: transparent;
    padding: 0;
  }
  :global(.prose ul) {
    list-style-type: disc;
    padding-left: 1.5rem;
  }
  :global(.prose ol) {
    list-style-type: decimal;
    padding-left: 1.5rem;
  }
  :global(.prose li) {
    margin-top: 0.25rem;
  }
  :global(.prose p) {
    margin-top: 0.5rem;
    margin-bottom: 0.5rem;
  }
</style>
