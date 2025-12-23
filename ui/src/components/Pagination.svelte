<script>
  let { total, limit, offset, onPageChange } = $props();

  const totalPages = $derived(Math.ceil(total / limit));
  const currentPage = $derived(Math.floor(offset / limit) + 1);

  // Calculate visible page numbers with sliding window
  const visiblePages = $derived.by(() => {
    const maxVisible = 7;
    const pages = [];

    if (totalPages <= maxVisible) {
      // Show all pages if total is less than max
      for (let i = 1; i <= totalPages; i++) {
        pages.push(i);
      }
    } else {
      // Sliding window logic
      let start = Math.max(1, currentPage - Math.floor(maxVisible / 2));
      let end = Math.min(totalPages, start + maxVisible - 1);

      // Adjust start if we're near the end
      if (end === totalPages) {
        start = Math.max(1, end - maxVisible + 1);
      }

      for (let i = start; i <= end; i++) {
        pages.push(i);
      }
    }

    return pages;
  });

  function goToPage(page) {
    const newOffset = (page - 1) * limit;
    onPageChange(newOffset);
  }
</script>

{#if totalPages > 1}
  <div class="join">
    <button
      class="join-item btn btn-sm"
      disabled={currentPage === 1}
      onclick={() => goToPage(currentPage - 1)}
    >
      «
    </button>

    {#if visiblePages[0] > 1}
      <button
        class="join-item btn btn-sm"
        onclick={() => goToPage(1)}
      >
        1
      </button>
      {#if visiblePages[0] > 2}
        <button class="join-item btn btn-sm btn-disabled">...</button>
      {/if}
    {/if}

    {#each visiblePages as page}
      <button
        class="join-item btn btn-sm"
        class:btn-active={page === currentPage}
        onclick={() => goToPage(page)}
      >
        {page}
      </button>
    {/each}

    {#if visiblePages[visiblePages.length - 1] < totalPages}
      {#if visiblePages[visiblePages.length - 1] < totalPages - 1}
        <button class="join-item btn btn-sm btn-disabled">...</button>
      {/if}
      <button
        class="join-item btn btn-sm"
        onclick={() => goToPage(totalPages)}
      >
        {totalPages}
      </button>
    {/if}

    <button
      class="join-item btn btn-sm"
      disabled={currentPage === totalPages}
      onclick={() => goToPage(currentPage + 1)}
    >
      »
    </button>
  </div>
{/if}
