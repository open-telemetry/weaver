<script>
  import { link } from 'svelte-spa-router';
  import { getAttribute } from '../lib/api.js';
  import StabilityBadge from '../components/StabilityBadge.svelte';
  import Markdown from '../components/Markdown.svelte';
  import InlineMarkdown from '../components/InlineMarkdown.svelte';

  let { params } = $props();

  let data = $state(null);
  let error = $state(null);

  $effect(() => {
    const key = params['*'] || params.wild;
    if (key) {
      getAttribute(key)
        .then(d => data = d)
        .catch(e => error = e.message);
    }
  });

  function formatType(type) {
    if (typeof type === 'string') return type;
    if (type?.members) {
      return 'enum { ' + type.members.map(m => m.value || m.id).join(', ') + ' }';
    }
    return JSON.stringify(type);
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
    <div class="flex items-center gap-4">
      <h1 class="text-2xl font-bold font-mono">{data.key}</h1>
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
          <div class="text-sm">{data.deprecated.note || 'This attribute is deprecated.'}</div>
          {#if data.deprecated.renamed_to}
            <div class="text-sm mt-1">Use <a href="/attributes/{data.deprecated.renamed_to}" use:link class="link">{data.deprecated.renamed_to}</a> instead.</div>
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

    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
      <div class="card bg-base-200">
        <div class="card-body">
          <h2 class="card-title">Type</h2>
          <code class="bg-base-300 p-2 rounded">{formatType(data.type)}</code>

          {#if data.type?.members}
            <div class="mt-4">
              <h3 class="font-semibold mb-2">Enum Values</h3>
              <div class="overflow-x-auto">
                <table class="table table-sm">
                  <thead>
                    <tr>
                      <th>Value</th>
                      <th>Description</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each data.type.members as member}
                      <tr>
                        <td class="font-mono">{member.value || member.id}</td>
                        <td><InlineMarkdown content={member.brief || '-'} /></td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            </div>
          {/if}
        </div>
      </div>

      <div class="card bg-base-200">
        <div class="card-body">
          <h2 class="card-title">Examples</h2>
          {#if data.examples?.length}
            <ul class="list-disc list-inside">
              {#each data.examples as example}
                <li class="font-mono">{JSON.stringify(example)}</li>
              {/each}
            </ul>
          {:else}
            <p class="text-base-content/70">No examples available.</p>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</div>
