<script>
  import { link } from "svelte-spa-router";
  import { search } from "../lib/api.js";
  import StabilityBadge from "../components/StabilityBadge.svelte";
  import InlineMarkdown from "../components/InlineMarkdown.svelte";

  const itemsPerPage = 50;

  let query = $state("");
  let searchType = $state("all");
  let stabilityFilter = $state(null);
  let currentPage = $state(1);
  let results = $state(null);
  let error = $state(null);
  let loading = $state(false);
  let initialized = $state(false);

  // Computed values
  let offset = $derived((currentPage - 1) * itemsPerPage);
  let totalPages = $derived(
    results ? Math.ceil(results.total / itemsPerPage) : 0,
  );

  // Load state from URL on mount
  $effect(() => {
    if (!initialized) {
      const params = new URLSearchParams(
        window.location.hash.split("?")[1] || "",
      );
      query = params.get("q") || "";
      searchType = params.get("type") || "all";
      stabilityFilter = params.get("stability") || null;
      currentPage = parseInt(params.get("page")) || 1;
      initialized = true;

      // Perform initial search if there are parameters
      if (query || searchType !== "all" || stabilityFilter) {
        performSearch();
      }
    }
  });

  // Update URL when state changes
  function updateURL() {
    const params = new URLSearchParams();
    if (query) params.set("q", query);
    if (searchType !== "all") params.set("type", searchType);
    if (stabilityFilter) params.set("stability", stabilityFilter);
    if (currentPage > 1) params.set("page", currentPage.toString());

    const queryString = params.toString();
    const newHash = queryString ? `#/search?${queryString}` : "#/search";

    if (window.location.hash !== newHash) {
      history.pushState(null, "", newHash);
    }
  }

  // Search as user types
  function handleQueryInput() {
    currentPage = 1;
    performSearch();
  }

  function handleKeyDown(e) {
    if (e.key === "Enter") {
      currentPage = 1;
      performSearch();
    }
  }

  async function performSearch() {
    loading = true;
    error = null;
    try {
      const q = query.trim() || null;
      const limit = itemsPerPage;
      const currentOffset = offset;

      results = await search(
        q,
        searchType,
        stabilityFilter,
        limit,
        currentOffset,
      );
      updateURL();
    } catch (e) {
      error = e.message;
      results = null;
    } finally {
      loading = false;
    }
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
    window.scrollTo({ top: 0, behavior: "smooth" });
  }

  function getItemLink(result) {
    switch (result.result_type) {
      case "attribute":
        return `/attributes/${result.key}`;
      case "metric":
        return `/metrics/${result.name}`;
      case "span":
        return `/spans/${result.type}`;
      case "event":
        return `/events/${result.name}`;
      case "entity":
        return `/entities/${result.type}`;
      default:
        return "#";
    }
  }

  function getItemId(result) {
    // For spans, entities: type is the ID
    // For metrics, events: name is the ID
    // For attributes: key is the ID
    if (result.result_type === "span" || result.result_type === "entity") {
      return result.type;
    }
    return result.key || result.name || result.type;
  }

  function formatType(type) {
    if (typeof type === "string") return type;
    if (type?.members) {
      return "enum";
    }
    if (type?.type) return type.type;
    return JSON.stringify(type);
  }

  function getTypeSpecificInfo(result) {
    switch (result.result_type) {
      case "attribute":
        return [{ label: "Type", value: formatType(result.type) }];
      case "metric":
        return [
          { label: "Instrument", value: result.instrument },
          { label: "Unit", value: result.unit || "-" },
        ];
      case "span":
        return [{ label: "Kind", value: result.kind || "-" }];
      case "event":
        return [];
      case "entity":
        return [];
      default:
        return [];
    }
  }
</script>

<div class="space-y-4">
  <h1 class="text-2xl font-bold">Search</h1>

  <div class="flex gap-4 flex-wrap">
    <input
      type="text"
      placeholder="Search attributes, metrics, spans, events, entities..."
      class="input input-bordered flex-1 min-w-64"
      bind:value={query}
      oninput={handleQueryInput}
      onkeydown={handleKeyDown}
    />
    <select
      class="select select-bordered"
      value={searchType}
      onchange={handleTypeChange}
    >
      <option value="all">All Types</option>
      <option value="attribute">Attributes</option>
      <option value="metric">Metrics</option>
      <option value="span">Spans</option>
      <option value="event">Events</option>
      <option value="entity">Entities</option>
    </select>
    <select
      class="select select-bordered"
      value={stabilityFilter || ""}
      onchange={handleStabilityChange}
    >
      <option value="">All Stability</option>
      <option value="stable">Stable</option>
      <option value="development">Development</option>
      <option value="alpha">Alpha</option>
      <option value="beta">Beta</option>
      <option value="release_candidate">Release Candidate</option>
      <option value="deprecated">Deprecated</option>
    </select>
  </div>

  {#if error}
    <div class="alert alert-error">
      <span>Error: {error}</span>
    </div>
  {:else if results}
    <div class="flex items-center justify-between">
      <p class="text-sm text-base-content/70">
        {#if results.query}
          Found {results.total} results for "{results.query}"
        {:else}
          Showing {results.count} of {results.total} items
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
    {:else}
      <div class="space-y-2">
        {#each results.results as result}
          <a
            href={getItemLink(result)}
            use:link
            class="card bg-base-200 hover:bg-base-300 cursor-pointer"
            class:opacity-50={result.deprecated}
          >
            <div class="card-body py-3">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="badge badge-outline">{result.result_type}</span>
                <span class="font-mono font-semibold">{getItemId(result)}</span>
                <StabilityBadge stability={result.stability} />
                {#if result.deprecated}
                  <span class="badge badge-sm badge-ghost">deprecated</span>
                {/if}
                {#each getTypeSpecificInfo(result) as info}
                  <span class="text-xs text-base-content/60">
                    <span class="font-semibold">{info.label}:</span>
                    {info.value}
                  </span>
                {/each}
                <span class="text-xs text-base-content/50 ml-auto"
                  >score: {result.score}</span
                >
              </div>
              <p class="text-sm text-base-content/70 truncate">
                <InlineMarkdown content={result.brief || "No description"} />
              </p>
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
      <p>Enter a search term or leave empty to browse all items.</p>
      <p class="text-sm mt-2">
        Use the type and stability filters to narrow results.
      </p>
    </div>
  {/if}
</div>
