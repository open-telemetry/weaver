<script>
  import { link } from "svelte-spa-router";
  import { getRegistryStats } from "../lib/api.js";

  let stats = $state(null);
  let error = $state(null);

  $effect(() => {
    getRegistryStats()
      .then((data) => (stats = data))
      .catch((e) => (error = e.message));
  });
</script>

<div class="space-y-6">
  <h1 class="text-3xl font-bold">Registry Stats</h1>

  {#if error}
    <div class="alert alert-error">
      <span>Error loading registry stats: {error}</span>
    </div>
  {:else if !stats}
    <div class="flex justify-center">
      <span class="loading loading-spinner loading-lg"></span>
    </div>
  {:else}
    {#if stats.registry_url}
      <p class="text-sm text-base-content/70">
        Source: <a href={stats.registry_url} target="_blank" class="link"
          >{stats.registry_url}</a
        >
      </p>
    {/if}

    <div class="stats stats-vertical lg:stats-horizontal shadow w-full">
      <a href="/search?type=attribute" use:link class="stat hover:bg-base-300 cursor-pointer transition-colors">
        <div class="stat-title">Attributes</div>
        <div class="stat-value">{stats.counts.attributes}</div>
        <div class="stat-desc">Semantic attributes</div>
      </a>

      <a href="/search?type=metric" use:link class="stat hover:bg-base-300 cursor-pointer transition-colors">
        <div class="stat-title">Metrics</div>
        <div class="stat-value">{stats.counts.metrics}</div>
        <div class="stat-desc">Metric definitions</div>
      </a>

      <a href="/search?type=span" use:link class="stat hover:bg-base-300 cursor-pointer transition-colors">
        <div class="stat-title">Spans</div>
        <div class="stat-value">{stats.counts.spans}</div>
        <div class="stat-desc">Span types</div>
      </a>

      <a href="/search?type=event" use:link class="stat hover:bg-base-300 cursor-pointer transition-colors">
        <div class="stat-title">Events</div>
        <div class="stat-value">{stats.counts.events}</div>
        <div class="stat-desc">Event definitions</div>
      </a>

      <a href="/search?type=entity" use:link class="stat hover:bg-base-300 cursor-pointer transition-colors">
        <div class="stat-title">Entities</div>
        <div class="stat-value">{stats.counts.entities}</div>
        <div class="stat-desc">Entity types</div>
      </a>
    </div>
  {/if}
</div>
