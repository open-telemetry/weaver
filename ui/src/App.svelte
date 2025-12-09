<script>
  import Router from "svelte-spa-router";
  import { link } from "svelte-spa-router";
  import Dashboard from "./routes/Dashboard.svelte";
  import Attributes from "./routes/Attributes.svelte";
  import AttributeDetail from "./routes/AttributeDetail.svelte";
  import Metrics from "./routes/Metrics.svelte";
  import MetricDetail from "./routes/MetricDetail.svelte";
  import Spans from "./routes/Spans.svelte";
  import SpanDetail from "./routes/SpanDetail.svelte";
  import Events from "./routes/Events.svelte";
  import EventDetail from "./routes/EventDetail.svelte";
  import Entities from "./routes/Entities.svelte";
  import EntityDetail from "./routes/EntityDetail.svelte";
  import Search from "./routes/Search.svelte";
  import Schema from "./routes/Schema.svelte";

  const routes = {
    "/": Dashboard,
    "/attributes": Attributes,
    "/attributes/*": AttributeDetail,
    "/metrics": Metrics,
    "/metrics/*": MetricDetail,
    "/spans": Spans,
    "/spans/*": SpanDetail,
    "/events": Events,
    "/events/*": EventDetail,
    "/entities": Entities,
    "/entities/*": EntityDetail,
    "/search": Search,
    "/schema": Schema,
  };

  let searchQuery = $state("");

  function handleSearch(e) {
    if (e.key === "Enter" && searchQuery.trim()) {
      window.location.hash = `/search?q=${encodeURIComponent(searchQuery)}`;
    }
  }
</script>

<div class="drawer lg:drawer-open">
  <input id="sidebar" type="checkbox" class="drawer-toggle" />

  <div class="drawer-content flex flex-col">
    <!-- Navbar -->
    <div class="navbar bg-base-200 sticky top-0 z-10">
      <div class="flex-none lg:hidden">
        <label for="sidebar" class="btn btn-square btn-ghost">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 24 24"
            class="inline-block w-6 h-6 stroke-current"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M4 6h16M4 12h16M4 18h16"
            ></path>
          </svg>
        </label>
      </div>
      <div class="flex-1">
        <a href="/" use:link class="btn btn-ghost text-xl">Weaver</a>
      </div>
      <div class="flex-none gap-2">
        <div class="form-control">
          <input
            type="text"
            placeholder="Search..."
            class="input input-bordered w-24 md:w-auto"
            bind:value={searchQuery}
            onkeydown={handleSearch}
          />
        </div>
      </div>
    </div>

    <!-- Main content -->
    <main class="flex-1 p-6">
      <Router {routes} />
    </main>
  </div>

  <!-- Sidebar -->
  <div class="drawer-side">
    <label for="sidebar" class="drawer-overlay"></label>
    <ul class="menu p-4 w-64 min-h-full bg-base-200 text-base-content">
      <li class="menu-title">Schema</li>
      <li><a href="/schema" use:link>Forge Resolved</a></li>

      <li class="menu-title mt-4">Registry</li>
      <li><a href="/" use:link>Dashboard</a></li>
      <li><a href="/search" use:link>Search</a></li>

      <li class="menu-title mt-4">Signals</li>
      <li><a href="/metrics" use:link>Metrics</a></li>
      <li><a href="/spans" use:link>Spans</a></li>
      <li><a href="/events" use:link>Events</a></li>
      <li><a href="/entities" use:link>Entities</a></li>

      <li class="menu-title mt-4">Definitions</li>
      <li><a href="/attributes" use:link>Attributes</a></li>
    </ul>
  </div>
</div>
