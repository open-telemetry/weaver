<script>
  import { link, querystring } from 'svelte-spa-router';
  import { search } from '../lib/api.js';
  import StabilityBadge from '../components/StabilityBadge.svelte';
  import InlineMarkdown from '../components/InlineMarkdown.svelte';

  // Initialize from URL query params
  const params = new URLSearchParams(window.location.hash.split('?')[1] || '');
  let query = $state(params.get('q') || '');
  let searchType = $state(params.get('type') || 'all');

  let results = $state(null);
  let error = $state(null);
  let loading = $state(false);

  // Update URL with current search state
  function updateURL() {
    const params = new URLSearchParams();
    if (query) params.set('q', query);
    if (searchType !== 'all') params.set('type', searchType);

    const queryString = params.toString();
    const hash = window.location.hash.split('?')[0];
    const newHash = queryString ? `${hash}?${queryString}` : hash;

    if (window.location.hash !== newHash) {
      history.replaceState(null, '', newHash);
    }
  }

  async function doSearch() {
    if (!query.trim()) {
      results = null;
      updateURL();
      return;
    }
    loading = true;
    error = null;
    try {
      results = await search(query, searchType, 100);
      updateURL();
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  // Load initial search results if query exists
  $effect(() => {
    if (query.trim()) {
      doSearch();
    }
  });

  function handleSubmit(e) {
    e.preventDefault();
    doSearch();
  }

  function handleTypeChange(e) {
    searchType = e.target.value;
    if (query.trim()) {
      doSearch();
    }
  }

  function getItemLink(result) {
    switch (result.kind) {
      case 'attribute': return `/attributes/${result.key}`;
      case 'metric': return `/metrics/${result.name}`;
      case 'span': return `/spans/${result.type}`;
      case 'event': return `/events/${result.name}`;
      case 'entity': return `/entities/${result.type}`;
      default: return '#';
    }
  }

  function getItemId(result) {
    return result.key || result.name || result.type;
  }
</script>

<div class="space-y-4">
  <h1 class="text-2xl font-bold">Search</h1>

  <form onsubmit={handleSubmit} class="flex gap-4">
    <input
      type="text"
      placeholder="Search attributes, metrics, spans..."
      class="input input-bordered flex-1"
      bind:value={query}
    />
    <select class="select select-bordered" value={searchType} onchange={handleTypeChange}>
      <option value="all">All Types</option>
      <option value="attribute">Attributes</option>
      <option value="metric">Metrics</option>
      <option value="span">Spans</option>
      <option value="event">Events</option>
      <option value="entity">Entities</option>
    </select>
    <button type="submit" class="btn btn-primary" disabled={loading}>
      {#if loading}
        <span class="loading loading-spinner loading-sm"></span>
      {:else}
        Search
      {/if}
    </button>
  </form>

  {#if error}
    <div class="alert alert-error">
      <span>Error: {error}</span>
    </div>
  {:else if results}
    <p class="text-sm text-base-content/70">
      Found {results.total} results for "{results.query}"
    </p>

    {#if results.results.length === 0}
      <div class="alert">
        <span>No results found. Try a different search term.</span>
      </div>
    {:else}
      <div class="space-y-2">
        {#each results.results as result}
          <a href={getItemLink(result)} use:link class="card bg-base-200 hover:bg-base-300 cursor-pointer" class:opacity-50={result.deprecated}>
            <div class="card-body py-3">
              <div class="flex items-center gap-2">
                <span class="badge badge-outline">{result.kind}</span>
                <span class="font-mono font-semibold">{getItemId(result)}</span>
                <StabilityBadge stability={result.stability} />
                {#if result.deprecated}
                  <span class="badge badge-sm badge-ghost">deprecated</span>
                {/if}
                <span class="text-xs text-base-content/50 ml-auto">score: {result.score}</span>
              </div>
              <p class="text-sm text-base-content/70 truncate"><InlineMarkdown content={result.brief || 'No description'} /></p>
            </div>
          </a>
        {/each}
      </div>
    {/if}
  {:else if !loading}
    <div class="text-center text-base-content/70 py-8">
      <p>Enter a search term to find attributes, metrics, spans, events, or entities.</p>
      <p class="text-sm mt-2">Try searching for "http", "duration", or "error".</p>
    </div>
  {/if}
</div>
