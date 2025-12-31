<script>
  import { onMount } from 'svelte';

  let rapidocElement;
  let currentTheme = $state('light');

  onMount(() => {
    // Get initial theme
    currentTheme = document.documentElement.getAttribute('data-theme') || 'light';

    // Update RapiDoc theme
    updateTheme(currentTheme);
  });

  function updateTheme(theme) {
    if (rapidocElement) {
      const isDark = theme === 'dark';
      rapidocElement.setAttribute('theme', isDark ? 'dark' : 'light');
      rapidocElement.setAttribute('bg-color', isDark ? '#1d232a' : '#ffffff');
      rapidocElement.setAttribute('text-color', isDark ? '#a6adba' : '#000000');
      rapidocElement.setAttribute('header-color', isDark ? '#1d232a' : '#f3f4f6');
      rapidocElement.setAttribute('primary-color', isDark ? '#3abff8' : '#0ea5e9');
      rapidocElement.setAttribute('nav-bg-color', isDark ? '#1d232a' : '#f3f4f6');
      rapidocElement.setAttribute('nav-text-color', isDark ? '#a6adba' : '#1f2937');
      rapidocElement.setAttribute('nav-hover-bg-color', isDark ? '#2a323c' : '#e5e7eb');
      rapidocElement.setAttribute('nav-hover-text-color', isDark ? '#ffffff' : '#000000');
      rapidocElement.setAttribute('nav-accent-color', isDark ? '#3abff8' : '#0ea5e9');
    }
  }

  // Watch for theme changes
  $effect(() => {
    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        if (mutation.attributeName === 'data-theme') {
          const newTheme = document.documentElement.getAttribute('data-theme');
          if (newTheme !== currentTheme) {
            currentTheme = newTheme;
            updateTheme(newTheme);
          }
        }
      }
    });

    observer.observe(document.documentElement, { attributes: true });
    return () => observer.disconnect();
  });
</script>

<svelte:head>
  <title>API Documentation - Weaver</title>
  <script type="module" src="https://unpkg.com/rapidoc/dist/rapidoc-min.js"></script>
</svelte:head>

<div class="api-docs-container">
  <rapi-doc
    bind:this={rapidocElement}
    spec-url="/api/v1/openapi.json"
    theme={currentTheme === 'dark' ? 'dark' : 'light'}
    bg-color={currentTheme === 'dark' ? '#1d232a' : '#ffffff'}
    text-color={currentTheme === 'dark' ? '#a6adba' : '#000000'}
    header-color={currentTheme === 'dark' ? '#1d232a' : '#f3f4f6'}
    primary-color={currentTheme === 'dark' ? '#3abff8' : '#0ea5e9'}
    nav-bg-color={currentTheme === 'dark' ? '#1d232a' : '#f3f4f6'}
    nav-text-color={currentTheme === 'dark' ? '#a6adba' : '#1f2937'}
    nav-hover-bg-color={currentTheme === 'dark' ? '#2a323c' : '#e5e7eb'}
    nav-hover-text-color={currentTheme === 'dark' ? '#ffffff' : '#000000'}
    nav-accent-color={currentTheme === 'dark' ? '#3abff8' : '#0ea5e9'}
    render-style="read"
    layout="column"
    schema-style="tree"
    show-header="false"
    allow-try="true"
    allow-server-selection="false"
    allow-authentication="false"
    style="height: 100%; width: 100%;"
  ></rapi-doc>
</div>

<style>
  .api-docs-container {
    position: fixed;
    top: 64px; /* Height of navbar */
    left: 256px; /* Width of sidebar on desktop */
    right: 0;
    bottom: 0;
    overflow: hidden;
  }

  @media (max-width: 1024px) {
    .api-docs-container {
      left: 0;
    }
  }

  :global(rapi-doc) {
    display: block;
    width: 100%;
    height: 100%;
  }

  :global(rapi-doc::part(section-navbar)) {
    overflow-y: auto !important;
    -webkit-overflow-scrolling: touch;
  }
</style>
