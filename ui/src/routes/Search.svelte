<script>
  import { link } from 'svelte-spa-router';
  import { search } from '../lib/api.js';
  import StabilityBadge from '../components/StabilityBadge.svelte';
  import InlineMarkdown from '../components/InlineMarkdown.svelte';

  // Initialize from URL query params
  const params = new URLSearchParams(window.location.hash.split('?')[1] || '');
  let query = $state(params.get('q') || '');
  let searchType = $state(params.get('type') || 'all');
  let stabilityFilter = $state(params.get('stability') || null);
  let currentPage = $state(parseInt(params.get('page')) || 1);
  const itemsPerPage = 25;

  let results = $state(null);
  let error = $state(null);
  let loading = $state(false);

  // Computed values
  let isSearchMode = $derived(query && query.trim().length > 0);
  let isBrowseMode = $derived(!isSearchMode && searchType !== 'all');
  let showTable = $derived(searchType !== 'all');
  let showScore = $derived(isSearchMode);
  let offset = $derived((currentPage - 1) * itemsPerPage);
  let totalPages = $derived(results ? Math.ceil(results.total / itemsPerPage) : 0);

  // Update URL with current state
  function updateURL() {
    const params = new URLSearchParams();
    if (query) params.set('q', query);
    if (searchType !== 'all') params.set('type', searchType);
    if (stabilityFilter) params.set('stability', stabilityFilter);
    if (currentPage > 1) params.set('page', currentPage.toString());

    const queryString = params.toString();
    const hash = window.location.hash.split('?')[0];
    const newHash = queryString ? `${hash}?${queryString}` : hash;

    if (window.location.hash !== newHash) {
      history.replaceState(null, '', newHash);
    }
  }

  async function performSearch() {
    loading = true;
    error = null;
    try {
      const q = query.trim() || null;
      const limit = itemsPerPage;
      const currentOffset = offset;

      results = await search(q, searchType, stabilityFilter, limit, currentOffset);
      updateURL();
    } catch (e) {
      error = e.message;
      results = null;
    } finally {
      loading = false;
    }
  }

  // Auto-load when type is selected or on mount if type is set
  $effect(() => {
    if (searchType !== 'all' || (query && query.trim())) {
      performSearch();
    }
  });

  function handleSubmit(e) {
    e.preventDefault();
    currentPage = 1; // Reset to first page on new search
    performSearch();
  }

  function handleTypeChange(e) {
    searchType = e.target.value;
    currentPage = 1;
    performSearch();
  }

  function handleStabilityChange(e) {
    stabilityFilter = e.target.value || null;
    currentPage = 1;
    performSearch();
  }

  function handlePageChange(newPage) {
    currentPage = newPage;
    performSearch();
    window.scrollTo({ top: 0, behavior: 'smooth' });
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

  function getColumnsForType(type) {
    switch(type) {
      case 'attribute':
        return ['Key', 'Type', 'Brief', 'Stability'];
      case 'metric':
        return ['Name', 'Instrument', 'Unit', 'Brief', 'Stability'];
      case 'span':
        return ['Type', 'Kind', 'Brief', 'Stability'];
      case 'event':
        return ['Name', 'Brief', 'Stability'];
      case 'entity':
        return ['Type', 'Brief', 'Stability'];
      default:
        return null;
    }
  }

  function getCellValue(result, column) {
    const columnLower = column.toLowerCase();
    switch(columnLower) {
      case 'key': return result.key;
      case 'type': return result.type?.type || result.type;
      case 'name': return result.name;
      case 'instrument': return result.instrument;
      case 'unit': return result.unit || '-';
      case 'kind': return result.span_kind || '-';
      case 'brief': return result.brief;
      case 'stability': return null; // Will be rendered as StabilityBadge
      default: return '';
    }
  }
</script>

<div class="space-y-4">
  <h1 class="text-2xl font-bold">
    {#if isBrowseMode}
      {searchType.charAt(0).toUpperCase() + searchType.slice(1)}s
    {:else}
      Search
    {/if}
  </h1>

  <form onsubmit={handleSubmit} class="flex gap-4 flex-wrap">
    <input
      type="text"
      placeholder="Search or leave empty to browse..."
      class="input input-bordered flex-1 min-w-64"
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
    <select class="select select-bordered" value={stabilityFilter || ''} onchange={handleStabilityChange}>
      <option value="">All Stability</option>
      <option value="stable">Stable</option>
      <option value="development">Development</option>
      <option value="alpha">Alpha</option>
      <option value="beta">Beta</option>
    </select>
    <button type="submit" class="btn btn-primary" disabled={loading}>
      {#if loading}
        <span class="loading loading-spinner loading-sm"></span>
      {:else}
        {isSearchMode ? 'Search' : 'Browse'}
      {/if}
    </button>
  </form>

  {#if error}
    <div class="alert alert-error">
      <span>Error: {error}</span>
    </div>
  {:else if results}
    <div class="flex items-center justify-between">
      <p class="text-sm text-base-content/70">
        {#if isSearchMode}
          Found {results.total} results for "{results.query}"
        {:else}
          Showing {results.count} of {results.total} {searchType === 'all' ? 'items' : searchType + 's'}
        {/if}
      </p>

      {#if totalPages > 1}
        <div class="join">
          <button
            class="join-item btn btn-sm"
            disabled={currentPage === 1}
            onclick={() => handlePageChange(currentPage - 1)}
          >
            «
          </button>
          <button class="join-item btn btn-sm">
            Page {currentPage} of {totalPages}
          </button>
          <button
            class="join-item btn btn-sm"
            disabled={currentPage === totalPages}
            onclick={() => handlePageChange(currentPage + 1)}
          >
            »
          </button>
        </div>
      {/if}
    </div>

    {#if results.results.length === 0}
      <div class="alert">
        <span>No results found. Try a different search term or filter.</span>
      </div>
    {:else if showTable}
      <!-- Table view for specific type -->
      <div class="overflow-x-auto">
        <table class="table table-zebra">
          <thead>
            <tr>
              {#each getColumnsForType(searchType) as column}
                <th>{column}</th>
              {/each}
              {#if showScore}
                <th class="text-right">Score</th>
              {/if}
            </tr>
          </thead>
          <tbody>
            {#each results.results as result}
              <tr class:opacity-50={result.deprecated}>
                {#each getColumnsForType(searchType) as column}
                  <td>
                    {#if column === 'Stability'}
                      <StabilityBadge stability={result.stability} />
                    {:else if column === getColumnsForType(searchType)[0]}
                      <!-- First column is the link -->
                      <a href={getItemLink(result)} use:link class="font-mono font-semibold hover:underline">
                        {getCellValue(result, column)}
                      </a>
                      {#if result.deprecated}
                        <span class="badge badge-sm badge-ghost ml-2">deprecated</span>
                      {/if}
                    {:else if column === 'Brief'}
                      <div class="text-sm text-base-content/70 max-w-md">
                        <InlineMarkdown content={getCellValue(result, column) || 'No description'} />
                      </div>
                    {:else}
                      {getCellValue(result, column)}
                    {/if}
                  </td>
                {/each}
                {#if showScore}
                  <td class="text-right text-xs text-base-content/50">{result.score}</td>
                {/if}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else}
      <!-- Card view for "All Types" -->
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
                {#if showScore}
                  <span class="text-xs text-base-content/50 ml-auto">score: {result.score}</span>
                {/if}
              </div>
              <p class="text-sm text-base-content/70 truncate"><InlineMarkdown content={result.brief || 'No description'} /></p>
            </div>
          </a>
        {/each}
      </div>
    {/if}

    <!-- Bottom pagination -->
    {#if totalPages > 1}
      <div class="flex justify-center mt-4">
        <div class="join">
          <button
            class="join-item btn btn-sm"
            disabled={currentPage === 1}
            onclick={() => handlePageChange(currentPage - 1)}
          >
            «
          </button>
          <button class="join-item btn btn-sm">
            Page {currentPage} of {totalPages}
          </button>
          <button
            class="join-item btn btn-sm"
            disabled={currentPage === totalPages}
            onclick={() => handlePageChange(currentPage + 1)}
          >
            »
          </button>
        </div>
      </div>
    {/if}
  {:else if !loading}
    <div class="text-center text-base-content/70 py-8">
      <p>Enter a search term to find items, or select a type to browse.</p>
      <p class="text-sm mt-2">Try searching for "http", "duration", or "error".</p>
    </div>
  {/if}
</div>
