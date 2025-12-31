<script>
  import { link } from 'svelte-spa-router';
  import { getEvent } from '../lib/api.js';
  import StabilityBadge from '../components/StabilityBadge.svelte';
  import Markdown from '../components/Markdown.svelte';
  import InlineMarkdown from '../components/InlineMarkdown.svelte';

  let { params } = $props();

  let data = $state(null);
  let error = $state(null);

  $effect(() => {
    const name = params['*'] || params.wild;
    if (name) {
      getEvent(name)
        .then(d => data = d)
        .catch(e => error = e.message);
    }
  });

  let copied = $state(false);

  function copyToClipboard(text) {
    navigator.clipboard.writeText(text).then(() => {
      copied = true;
      setTimeout(() => copied = false, 2000);
    });
  }
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
    <div class="flex items-center gap-4 flex-wrap">
      <h1 class="text-2xl font-bold font-mono">{data.name}</h1>
      <button
        class="btn btn-ghost btn-sm btn-circle"
        onclick={() => copyToClipboard(data.name)}
        title="Copy to clipboard"
      >
        {#if copied}
          <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
          </svg>
        {:else}
          <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
          </svg>
        {/if}
      </button>
      <span class="badge badge-outline">Event</span>
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
          <div class="text-sm">{data.deprecated.note || 'This event is deprecated.'}</div>
          {#if data.deprecated.renamed_to}
            <div class="text-sm mt-1">Use <a href="/event/{data.deprecated.renamed_to}" use:link class="link">{data.deprecated.renamed_to}</a> instead.</div>
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

    {#if data.attributes?.length}
      <div class="card bg-base-200">
        <div class="card-body">
          <h2 class="card-title">Event Attributes</h2>
          <div class="overflow-x-auto">
            <table class="table">
              <thead>
                <tr>
                  <th>Attribute</th>
                  <th>Type</th>
                  <th>Requirement</th>
                  <th>Brief</th>
                </tr>
              </thead>
              <tbody>
                {#each data.attributes as attr}
                  <tr>
                    <td>
                      <a href="/attribute/{attr.key}" use:link class="link link-primary font-mono text-sm">
                        {attr.key}
                      </a>
                    </td>
                    <td class="font-mono text-sm">{typeof attr.type === 'string' ? attr.type : 'enum'}</td>
                    <td>
                      {#if typeof attr.requirement_level === 'string'}
                        <span class="badge" class:badge-error={attr.requirement_level === 'required'}>
                          {attr.requirement_level}
                        </span>
                      {:else if attr.requirement_level?.conditionally_required}
                        <span class="badge badge-warning">conditionally required</span>
                      {:else}
                        <span class="badge">optional</span>
                      {/if}
                    </td>
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
