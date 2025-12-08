<script>
  import { link } from 'svelte-spa-router';
  import { getAttributes } from '../lib/api.js';
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
      data = await getAttributes({ stability: stability || undefined, limit, offset });
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

  function formatType(type) {
    if (typeof type === 'string') return type;
    if (type?.members) return 'enum';
    return JSON.stringify(type);
  }
</script>

<div class="space-y-4">
  <div class="flex justify-between items-center">
    <h1 class="text-2xl font-bold">Attributes</h1>
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
      Showing {data.count} of {data.total} attributes
    </p>

    <div class="overflow-x-auto">
      <table class="table table-zebra">
        <thead>
          <tr>
            <th>Key</th>
            <th>Type</th>
            <th>Brief</th>
            <th>Stability</th>
          </tr>
        </thead>
        <tbody>
          {#each data.items as attr}
            <tr class="hover" class:opacity-50={attr.deprecated}>
              <td>
                <a href="/attributes/{attr.key}" use:link class="link link-primary font-mono text-sm">
                  {attr.key}
                </a>
                {#if attr.deprecated}
                  <span class="badge badge-sm badge-ghost ml-2">deprecated</span>
                {/if}
              </td>
              <td class="font-mono text-sm">{formatType(attr.type)}</td>
              <td class="max-w-md truncate"><InlineMarkdown content={attr.brief || '-'} /></td>
              <td><StabilityBadge stability={attr.stability} /></td>
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
