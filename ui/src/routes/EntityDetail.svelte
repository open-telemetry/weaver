<script>
  import { link } from 'svelte-spa-router';
  import { getEntity } from '../lib/api.js';
  import StabilityBadge from '../components/StabilityBadge.svelte';
  import Markdown from '../components/Markdown.svelte';
  import InlineMarkdown from '../components/InlineMarkdown.svelte';

  let { params } = $props();

  let data = $state(null);
  let error = $state(null);

  $effect(() => {
    const type = params['*'] || params.wild;
    if (type) {
      getEntity(type)
        .then(d => data = d)
        .catch(e => error = e.message);
    }
  });
</script>

<div class="space-y-4">
  {#if error}
    <div class="alert alert-error">
      <span>Error: {error}</span>
    </div>
  {:else if !data}
    <div class="flex justify-center">
      <span class="loading loading-spinner loading-lg"></span>
    </div>
  {:else}
    <div class="flex items-center gap-4">
      <h1 class="text-2xl font-bold font-mono">{data.type}</h1>
      <StabilityBadge stability={data.stability} />
      {#if data.deprecated}
        <span class="badge badge-warning">deprecated</span>
      {/if}
    </div>

    {#if data.deprecated}
      <div class="alert alert-warning">
        <svg xmlns="http://www.w3.org/2000/svg" class="stroke-current shrink-0 h-6 w-6" fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" /></svg>
        <div>
          <div class="font-bold">Deprecated</div>
          <div class="text-sm">{data.deprecated.note || 'This entity is deprecated.'}</div>
          {#if data.deprecated.renamed_to}
            <div class="text-sm mt-1">Use <a href="/entity/{data.deprecated.renamed_to}" use:link class="link">{data.deprecated.renamed_to}</a> instead.</div>
          {/if}
        </div>
      </div>
    {/if}

    <div class="card bg-base-200">
      <div class="card-body">
        <h2 class="card-title">Description</h2>
        <div class="text-sm">
          <Markdown content={data.brief || 'No description available.'} />
        </div>
        {#if data.note}
          <div class="mt-4">
            <h3 class="font-semibold">Note</h3>
            <div class="text-sm">
              <Markdown content={data.note} />
            </div>
          </div>
        {/if}
      </div>
    </div>

    {#if data.identity?.length}
      <div class="card bg-base-200">
        <div class="card-body">
          <h2 class="card-title">Identity Attributes</h2>
          <div class="overflow-x-auto">
            <table class="table">
              <thead>
                <tr>
                  <th>Attribute</th>
                  <th>Type</th>
                  <th>Brief</th>
                </tr>
              </thead>
              <tbody>
                {#each data.identity as attr}
                  <tr>
                    <td>
                      <a href="/attribute/{attr.key}" use:link class="link link-primary font-mono text-sm">
                        {attr.key}
                      </a>
                    </td>
                    <td class="font-mono text-sm">{typeof attr.type === 'string' ? attr.type : 'enum'}</td>
                    <td class="max-w-xs truncate"><InlineMarkdown content={attr.brief || '-'} /></td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    {/if}

    {#if data.description?.length}
      <div class="card bg-base-200">
        <div class="card-body">
          <h2 class="card-title">Description Attributes</h2>
          <div class="overflow-x-auto">
            <table class="table">
              <thead>
                <tr>
                  <th>Attribute</th>
                  <th>Type</th>
                  <th>Brief</th>
                </tr>
              </thead>
              <tbody>
                {#each data.description as attr}
                  <tr>
                    <td>
                      <a href="/attribute/{attr.key}" use:link class="link link-primary font-mono text-sm">
                        {attr.key}
                      </a>
                    </td>
                    <td class="font-mono text-sm">{typeof attr.type === 'string' ? attr.type : 'enum'}</td>
                    <td class="max-w-xs truncate"><InlineMarkdown content={attr.brief || '-'} /></td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    {/if}
  {/if}
</div>
