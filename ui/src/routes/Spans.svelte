<script>
  import { link } from 'svelte-spa-router';
  import { getSpans } from '../lib/api.js';
  import StabilityBadge from '../components/StabilityBadge.svelte';
  import Pagination from '../components/Pagination.svelte';
  import InlineMarkdown from '../components/InlineMarkdown.svelte';

  // Initialize from URL query params
  const params = new URLSearchParams(window.location.hash.split('?')[1] || '');
  let stability = $state(params.get('stability') || '');
  let offset = $state(parseInt(params.get('offset') || '0', 10));
  const limit = 25;

  let data = $state(null);
  let error = $state(null);

  // Update URL with current state
  function updateURL() {
    const params = new URLSearchParams();
    if (stability) params.set('stability', stability);
    if (offset > 0) params.set('offset', offset.toString());

    const queryString = params.toString();
    const hash = window.location.hash.split('?')[0];
    const newHash = queryString ? `${hash}?${queryString}` : hash;

    if (window.location.hash !== newHash) {
      history.replaceState(null, '', newHash);
    }
  }

  async function loadData() {
    try {
      data = await getSpans({ stability: stability || undefined, limit, offset });
      updateURL();
    } catch (e) {
      error = e.message;
    }
  }

  // Load data once on mount
  $effect(() => {
    loadData();
  });

  function handleStabilityChange(e) {
    stability = e.target.value;
    offset = 0;
    loadData();
  }

  function handlePageChange(newOffset) {
    offset = newOffset;
    loadData();
  }
</script>

<div class="space-y-4">
  <div class="flex justify-between items-center">
    <h1 class="text-2xl font-bold">Spans</h1>
    <select class="select select-bordered" value={stability} onchange={handleStabilityChange}>
      <option value="">All Stability</option>
      <option value="stable">Stable</option>
      <option value="development">Development</option>
      <option value="alpha">Alpha</option>
      <option value="beta">Beta</option>
    </select>
  </div>

  {#if error}
    <div class="alert alert-error">
      <span>Error: {error}</span>
    </div>
  {:else if !data}
    <div class="flex justify-center">
      <span class="loading loading-spinner loading-lg"></span>
    </div>
  {:else}
    <p class="text-sm text-base-content/70">
      Showing {data.count} of {data.total} spans
    </p>

    <div class="overflow-x-auto">
      <table class="table table-zebra">
        <thead>
          <tr>
            <th>Type</th>
            <th>Kind</th>
            <th>Brief</th>
            <th>Stability</th>
          </tr>
        </thead>
        <tbody>
          {#each data.items as span}
            <tr class="hover" class:opacity-50={span.deprecated}>
              <td>
                <a href="/spans/{span.type}" use:link class="link link-primary font-mono text-sm">
                  {span.type}
                </a>
                {#if span.deprecated}
                  <span class="badge badge-sm badge-ghost ml-2">deprecated</span>
                {/if}
              </td>
              <td><span class="badge badge-outline">{span.kind || '-'}</span></td>
              <td class="max-w-md truncate"><InlineMarkdown content={span.brief || '-'} /></td>
              <td><StabilityBadge stability={span.stability} /></td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    <div class="flex justify-center">
      <Pagination total={data.total} {limit} {offset} onPageChange={handlePageChange} />
    </div>
  {/if}
</div>
