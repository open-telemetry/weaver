<script>
  import { link } from 'svelte-spa-router';
  import { getRegistry } from '../lib/api.js';

  let registry = $state(null);
  let error = $state(null);

  $effect(() => {
    getRegistry()
      .then(data => registry = data)
      .catch(e => error = e.message);
  });
</script>

<div class="space-y-6">
  <h1 class="text-3xl font-bold">Registry Overview</h1>

  {#if error}
    <div class="alert alert-error">
      <span>Error loading registry: {error}</span>
    </div>
  {:else if !registry}
    <div class="flex justify-center">
      <span class="loading loading-spinner loading-lg"></span>
    </div>
  {:else}
    {#if registry.registry_url}
      <p class="text-sm text-base-content/70">
        Source: <a href={registry.registry_url} target="_blank" class="link">{registry.registry_url}</a>
      </p>
    {/if}

    <div class="stats stats-vertical lg:stats-horizontal shadow w-full">
      <a href="/attributes" use:link class="stat hover:bg-base-200 cursor-pointer">
        <div class="stat-title">Attributes</div>
        <div class="stat-value">{registry.counts.attributes}</div>
        <div class="stat-desc">Semantic attributes</div>
      </a>

      <a href="/metrics" use:link class="stat hover:bg-base-200 cursor-pointer">
        <div class="stat-title">Metrics</div>
        <div class="stat-value">{registry.counts.metrics}</div>
        <div class="stat-desc">Metric definitions</div>
      </a>

      <a href="/spans" use:link class="stat hover:bg-base-200 cursor-pointer">
        <div class="stat-title">Spans</div>
        <div class="stat-value">{registry.counts.spans}</div>
        <div class="stat-desc">Span types</div>
      </a>

      <a href="/events" use:link class="stat hover:bg-base-200 cursor-pointer">
        <div class="stat-title">Events</div>
        <div class="stat-value">{registry.counts.events}</div>
        <div class="stat-desc">Event definitions</div>
      </a>

      <a href="/entities" use:link class="stat hover:bg-base-200 cursor-pointer">
        <div class="stat-title">Entities</div>
        <div class="stat-value">{registry.counts.entities}</div>
        <div class="stat-desc">Entity types</div>
      </a>
    </div>

    <div class="card bg-base-200">
      <div class="card-body">
        <h2 class="card-title">Quick Start</h2>
        <p>Explore the semantic conventions registry:</p>
        <ul class="list-disc list-inside space-y-1 mt-2">
          <li>Browse <a href="/attributes" use:link class="link link-primary">Attributes</a> to see available semantic attributes</li>
          <li>Check <a href="/metrics" use:link class="link link-primary">Metrics</a> for metric definitions and their attributes</li>
          <li>View <a href="/spans" use:link class="link link-primary">Spans</a> for tracing span conventions</li>
          <li>Use <a href="/search" use:link class="link link-primary">Search</a> to find specific items</li>
        </ul>
      </div>
    </div>
  {/if}
</div>
