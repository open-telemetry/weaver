<script>
  import { link } from "svelte-spa-router";
  import { getRegistry } from "../lib/api.js";

  let registry = $state(null);
  let error = $state(null);

  $effect(() => {
    getRegistry()
      .then((data) => (registry = data))
      .catch((e) => (error = e.message));
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
        Source: <a href={registry.registry_url} target="_blank" class="link"
          >{registry.registry_url}</a
        >
      </p>
    {/if}

    <div class="stats stats-vertical lg:stats-horizontal shadow w-full">
      <div class="stat">
        <div class="stat-title">Attributes</div>
        <div class="stat-value">{registry.counts.attributes}</div>
        <div class="stat-desc">Semantic attributes</div>
      </div>

      <div class="stat">
        <div class="stat-title">Metrics</div>
        <div class="stat-value">{registry.counts.metrics}</div>
        <div class="stat-desc">Metric definitions</div>
      </div>

      <div class="stat">
        <div class="stat-title">Spans</div>
        <div class="stat-value">{registry.counts.spans}</div>
        <div class="stat-desc">Span types</div>
      </div>

      <div class="stat">
        <div class="stat-title">Events</div>
        <div class="stat-value">{registry.counts.events}</div>
        <div class="stat-desc">Event definitions</div>
      </div>

      <div class="stat">
        <div class="stat-title">Entities</div>
        <div class="stat-value">{registry.counts.entities}</div>
        <div class="stat-desc">Entity types</div>
      </div>
    </div>
  {/if}
</div>
