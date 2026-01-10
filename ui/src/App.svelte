<script>
  import Router from "svelte-spa-router";
  import { link, location } from "svelte-spa-router";
  import Dashboard from "./routes/Dashboard.svelte";
  import AttributeDetail from "./routes/AttributeDetail.svelte";
  import MetricDetail from "./routes/MetricDetail.svelte";
  import SpanDetail from "./routes/SpanDetail.svelte";
  import EventDetail from "./routes/EventDetail.svelte";
  import EntityDetail from "./routes/EntityDetail.svelte";
  import Search from "./routes/Search.svelte";
  import Schema from "./routes/Schema.svelte";
  import ApiDocs from "./routes/ApiDocs.svelte";

  const routes = {
    "/": Search,
    "/search": Search,
    "/stats": Dashboard,
    "/attribute/*": AttributeDetail,
    "/metric/*": MetricDetail,
    "/span/*": SpanDetail,
    "/event/*": EventDetail,
    "/entity/*": EntityDetail,
    "/schema": Schema,
    "/api-docs": ApiDocs,
  };

  let theme = $state("light");
  let currentPath = $state("");

  // Track current location
  $effect(() => {
    const unsubscribe = location.subscribe(value => {
      currentPath = value;
    });
    return unsubscribe;
  });

  function isActive(path) {
    if (path === "/") return currentPath === "/";
    return currentPath.startsWith(path);
  }

  // Load theme from localStorage on mount
  $effect(() => {
    const savedTheme = localStorage.getItem("theme") || "light";
    theme = savedTheme;
    document.documentElement.setAttribute("data-theme", savedTheme);
  });

  function toggleTheme() {
    const newTheme = theme === "light" ? "dark" : "light";
    theme = newTheme;
    localStorage.setItem("theme", newTheme);
    document.documentElement.setAttribute("data-theme", newTheme);
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
        <button
          class="btn btn-ghost btn-circle"
          onclick={toggleTheme}
          title="Toggle theme"
        >
          {#if theme === "light"}
            <svg
              xmlns="http://www.w3.org/2000/svg"
              class="h-6 w-6"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"
              />
            </svg>
          {:else}
            <svg
              xmlns="http://www.w3.org/2000/svg"
              class="h-6 w-6"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"
              />
            </svg>
          {/if}
        </button>
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
      <li class="menu-title">Registry</li>
      <li><a href="/" use:link class:active={isActive("/") && currentPath === "/"}>Search</a></li>
      <li><a href="/stats" use:link class:active={isActive("/stats")}>Stats</a></li>
      <li class="menu-title mt-4">Schema</li>
      <li><a href="/schema?schema=MaterializedRegistryV2" use:link class:active={isActive("/schema") && currentPath.includes("MaterializedRegistryV2")}>MaterializedRegistryV2</a></li>
      <li><a href="/schema?schema=SemconvDefinitionV2" use:link class:active={isActive("/schema") && currentPath.includes("SemconvDefinitionV2")}>SemconvDefinitionV2</a></li>
      <li><a href="/schema?schema=LiveCheckSample" use:link class:active={isActive("/schema") && currentPath.includes("LiveCheckSample")}>LiveCheckSample</a></li>
      <li class="menu-title mt-4">Developer</li>
      <li><a href="/api-docs" use:link class:active={isActive("/api-docs")}>API Documentation</a></li>
    </ul>
  </div>
</div>
