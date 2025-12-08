<script>
  let { stability } = $props();

  const badgeClass = $derived({
    'stable': 'badge-success',
    'development': 'badge-warning',
    'alpha': 'badge-info',
    'beta': 'badge-info',
  }[stability] || 'badge-ghost');
</script>

{#if stability}
  <span class="badge {badgeClass}">{stability}</span>
{/if}
