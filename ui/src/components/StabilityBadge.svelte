<script>
  let { stability } = $props();

  const badgeClass = $derived({
    'stable': 'badge-success',
    'development': 'badge-warning',
    'alpha': 'badge-info',
    'beta': 'badge-info',
    'release_candidate': 'badge-accent',
    'deprecated': 'badge-error',
  }[stability] || 'badge-ghost');

  const label = $derived({
    'stable': 'Stable',
    'development': 'Development',
    'alpha': 'Alpha',
    'beta': 'Beta',
    'release_candidate': 'Release Candidate',
    'deprecated': 'Deprecated',
  }[stability] || stability);
</script>

{#if stability}
  <span class="badge {badgeClass}">{label}</span>
{/if}
