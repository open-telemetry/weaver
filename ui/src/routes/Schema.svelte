<script>
  import Markdown from '../components/Markdown.svelte';
  import InlineMarkdown from '../components/InlineMarkdown.svelte';

  let schema = $state(null);
  let error = $state(null);
  let loading = $state(true);
  let selectedDefinition = $state(null);
  let showRoot = $state(true);

  // Fetch schema on mount
  $effect(() => {
    async function fetchSchema() {
      try {
        const response = await fetch('/api/v1/schema');
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        schema = await response.json();
      } catch (e) {
        error = e.message;
      } finally {
        loading = false;
      }
    }
    fetchSchema();
  });

  // Initialize state from URL on mount and listen for popstate
  $effect(() => {
    function updateFromURL() {
      const params = new URLSearchParams(window.location.search);
      const type = params.get('type');

      if (type === 'root' || !type) {
        showRoot = true;
        selectedDefinition = null;
      } else {
        showRoot = false;
        selectedDefinition = type;
      }
    }

    // Initialize from URL
    updateFromURL();

    // Listen for browser back/forward
    window.addEventListener('popstate', updateFromURL);

    return () => {
      window.removeEventListener('popstate', updateFromURL);
    };
  });

  // Extract definitions from schema
  const definitions = $derived(schema?.definitions ? Object.keys(schema.definitions).sort() : []);

  function selectDefinition(name) {
    selectedDefinition = name;
    showRoot = false;
    // Update URL with new state
    const url = new URL(window.location.href);
    url.searchParams.set('type', name);
    window.history.pushState({}, '', url);
  }

  function selectRoot() {
    showRoot = true;
    selectedDefinition = null;
    // Update URL with new state
    const url = new URL(window.location.href);
    url.searchParams.set('type', 'root');
    window.history.pushState({}, '', url);
  }

  function formatType(prop, skipNull = false) {
    // Check for array with items first (before checking prop.type)
    if (prop.type === 'array' && prop.items) {
      // Handle nested arrays (array of array)
      if (prop.items.type === 'array' && prop.items.items) {
        const innerType = prop.items.items.$ref
          ? prop.items.items.$ref.replace('#/definitions/', '')
          : prop.items.items.type || 'any';
        return `array of array of ${innerType}`;
      }
      // Handle simple arrays
      const itemType = prop.items.$ref
        ? prop.items.$ref.replace('#/definitions/', '')
        : prop.items.type || 'any';
      return `array of ${itemType}`;
    }

    // Check for object with additionalProperties (like a map/dictionary)
    // Handle both type: "object" and type: ["object", "null"]
    const hasObjectType = prop.type === 'object' ||
                         (Array.isArray(prop.type) && prop.type.includes('object'));
    if (hasObjectType && prop.additionalProperties) {
      const valueType = prop.additionalProperties.$ref
        ? prop.additionalProperties.$ref.replace('#/definitions/', '')
        : prop.additionalProperties.type || 'any';

      // If it's a union type like ["object", "null"], filter out null if skipNull is true
      if (Array.isArray(prop.type) && prop.type.length > 1) {
        const otherTypes = prop.type.filter(t => t !== 'object' && (!skipNull || t !== 'null'));
        if (otherTypes.length > 0) {
          return `map of ${valueType} | ${otherTypes.join(' | ')}`;
        }
      }
      return `map of ${valueType}`;
    }

    if (Array.isArray(prop.type)) {
      const types = skipNull ? prop.type.filter(t => t !== 'null') : prop.type;
      return types.join(' | ');
    }
    if (prop.type) return prop.type;
    if (prop.$ref) return prop.$ref.replace('#/definitions/', '');
    if (prop.allOf) {
      // allOf is typically used for single type references
      if (prop.allOf.length === 1 && prop.allOf[0].$ref) {
        return prop.allOf[0].$ref.replace('#/definitions/', '');
      }
      return prop.allOf.map(t => t.$ref ? t.$ref.replace('#/definitions/', '') : t.type || 'object').join(' & ');
    }
    if (prop.anyOf) {
      const types = prop.anyOf.map(t => t.$ref ? t.$ref.replace('#/definitions/', '') : t.type || 'null');
      const filtered = skipNull ? types.filter(t => t !== 'null') : types;
      return filtered.join(' | ');
    }
    if (prop.oneOf) {
      const types = prop.oneOf.map(t => t.$ref ? t.$ref.replace('#/definitions/', '') : t.type || 'null');
      const filtered = skipNull ? types.filter(t => t !== 'null') : types;
      return filtered.join(' | ');
    }
    return 'unknown';
  }

  // Check if a type string is a reference to a definition
  function isDefinitionRef(typeStr) {
    if (!typeStr || typeStr === 'unknown') return false;
    // Check if it's a simple type
    const simpleTypes = ['string', 'number', 'boolean', 'object', 'array', 'null', 'integer', 'any'];
    if (simpleTypes.includes(typeStr)) return false;
    // Check if it starts with "array of " (array type)
    if (typeStr.startsWith('array of ')) return false;
    // Check if it starts with "map of " (map type)
    if (typeStr.startsWith('map of ')) return false;
    // Check if it contains " | " (union type)
    if (typeStr.includes(' | ')) return false;
    // Otherwise, it's likely a definition reference
    return schema?.definitions && schema.definitions[typeStr] !== undefined;
  }

  // Parse a type string and return clickable parts
  function parseTypeString(typeStr) {
    if (!typeStr) return [];

    // Handle union types (e.g., "Stability | null" or "map of YamlValue | null")
    if (typeStr.includes(' | ')) {
      const parts = typeStr.split(' | ');
      const result = [];

      parts.forEach((part, index) => {
        const trimmedPart = part.trim();

        // Check if this part is a map/array type
        if (trimmedPart.startsWith('map of ')) {
          const valueType = trimmedPart.slice(7);
          result.push({ text: 'map of ', isClickable: false });
          result.push({ text: valueType, isClickable: isDefinitionRef(valueType) });
        } else if (trimmedPart.startsWith('array of array of ')) {
          const innerType = trimmedPart.slice(18);
          result.push({ text: 'array of array of ', isClickable: false });
          result.push({ text: innerType, isClickable: isDefinitionRef(innerType) });
        } else if (trimmedPart.startsWith('array of ')) {
          const innerType = trimmedPart.slice(9);
          result.push({ text: 'array of ', isClickable: false });
          result.push({ text: innerType, isClickable: isDefinitionRef(innerType) });
        } else {
          result.push({ text: trimmedPart, isClickable: isDefinitionRef(trimmedPart) });
        }

        // Add the separator if not the last part
        if (index < parts.length - 1) {
          result.push({ text: ' | ', isClickable: false });
        }
      });

      return result;
    }

    // Handle nested array types (e.g., "array of array of integer")
    if (typeStr.startsWith('array of array of ')) {
      const innerType = typeStr.slice(18); // Skip "array of array of "
      return [
        { text: 'array of array of ', isClickable: false },
        { text: innerType, isClickable: isDefinitionRef(innerType) }
      ];
    }

    // Handle simple array types (e.g., "array of AttributeDef")
    if (typeStr.startsWith('array of ')) {
      const innerType = typeStr.slice(9); // Skip "array of "
      return [
        { text: 'array of ', isClickable: false },
        { text: innerType, isClickable: isDefinitionRef(innerType) }
      ];
    }

    // Handle map types (e.g., "map of YamlValue")
    if (typeStr.startsWith('map of ')) {
      const valueType = typeStr.slice(7); // Skip "map of "
      return [
        { text: 'map of ', isClickable: false },
        { text: valueType, isClickable: isDefinitionRef(valueType) }
      ];
    }

    // Single type
    return [{ text: typeStr, isClickable: isDefinitionRef(typeStr) }];
  }

</script>

<div class="flex gap-4 h-[calc(100vh-4rem)]">
  <!-- Left sidebar: definitions list -->
  <div class="w-80 border-r border-base-300 overflow-y-auto pr-4">
    {#if loading}
      <div class="flex justify-center py-8">
        <span class="loading loading-spinner loading-lg"></span>
      </div>
    {:else if error}
      <div class="alert alert-error">
        <span>Error: {error}</span>
      </div>
    {:else if schema}
      <div class="space-y-4">
        <!-- Schema info -->
        <button
          class="card sticky top-0 z-10 w-full text-left transition-colors cursor-pointer"
          class:bg-primary={showRoot}
          class:text-primary-content={showRoot}
          class:hover:bg-primary-focus={showRoot}
          class:bg-base-200={!showRoot}
          class:hover:bg-base-300={!showRoot}
          onclick={() => selectRoot()}
        >
          <div class="card-body p-4">
            <h1 class="font-bold text-xl">{schema.title || 'Schema'}</h1>
            <p class="text-sm opacity-70">{schema.description || ''}</p>
          </div>
        </button>

        <!-- Definitions -->
        <div>
          <h3 class="font-bold text-sm mb-2 text-base-content/70">TYPE DEFINITIONS ({definitions.length})</h3>
          <div class="space-y-1">
            {#each definitions as def}
              <button
                class="w-full text-left px-3 py-2 rounded transition-colors text-sm"
                class:bg-primary={selectedDefinition === def}
                class:text-primary-content={selectedDefinition === def}
                class:hover:bg-primary-focus={selectedDefinition === def}
                class:hover:bg-base-200={selectedDefinition !== def}
                onclick={() => selectDefinition(def)}
              >
                <code class="font-mono">{def}</code>
              </button>
            {/each}
          </div>
        </div>
      </div>
    {/if}
  </div>

  <!-- Right panel: definition details -->
  <div class="flex-1 overflow-y-auto">
    {#if showRoot && schema?.properties}
      <div class="space-y-6">
        <!-- Header -->
        <div>
          <h2 class="text-2xl font-mono font-bold mb-2">Root</h2>
          <div class="text-base-content/70">
            <p>Top-level properties of the schema</p>
          </div>
          <div class="mt-2">
            <span class="badge badge-outline">object</span>
          </div>
        </div>

        <!-- Required fields -->
        {#if schema.required && schema.required.length > 0}
          <div>
            <h3 class="text-lg font-bold mb-3">Required Fields</h3>
            <div class="flex flex-wrap gap-2">
              {#each schema.required as field}
                <span class="badge badge-error">{field}</span>
              {/each}
            </div>
          </div>
        {/if}

        <!-- Properties -->
        <div>
          <h3 class="text-lg font-bold mb-3">Properties</h3>
          <div class="overflow-x-auto">
            <table class="table table-zebra">
              <thead>
                <tr>
                  <th>Field</th>
                  <th>Type</th>
                  <th>Required</th>
                  <th>Description</th>
                </tr>
              </thead>
              <tbody>
                {#each Object.entries(schema.properties) as [propName, propDef]}
                  {@const isRequired = schema.required?.includes(propName)}
                  {@const typeStr = formatType(propDef, !isRequired)}
                  {@const typeParts = parseTypeString(typeStr)}
                  <tr>
                    <td>
                      <code class="font-mono text-sm">{propName}</code>
                    </td>
                    <td>
                      <span class="badge badge-sm badge-outline font-mono inline-flex items-center gap-1">
                        {#each typeParts as part}
                          {#if part.isClickable}
                            <button
                              class="hover:text-primary cursor-pointer underline"
                              onclick={() => selectDefinition(part.text)}
                            >
                              {part.text}
                            </button>
                          {:else}
                            <span>{part.text}</span>
                          {/if}
                        {/each}
                      </span>
                    </td>
                    <td>
                      {#if isRequired}
                        <span class="badge badge-sm badge-error">required</span>
                      {:else}
                        <span class="badge badge-sm badge-ghost">optional</span>
                      {/if}
                    </td>
                    <td class="text-sm max-w-md">
                      {#if propDef.description}
                        <InlineMarkdown content={propDef.description} />
                      {:else}
                        -
                      {/if}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>

        <!-- Raw JSON view -->
        <div>
          <h3 class="text-lg font-bold mb-3">Raw JSON</h3>
          <div class="mockup-code bg-base-300">
            <pre class="text-xs overflow-x-auto text-base-content"><code>{JSON.stringify({
  type: 'object',
  properties: schema.properties,
  required: schema.required
}, null, 2)}</code></pre>
          </div>
        </div>
      </div>
    {:else if selectedDefinition && schema?.definitions[selectedDefinition]}
      {@const def = schema.definitions[selectedDefinition]}
      <div class="space-y-6">
        <!-- Header -->
        <div>
          <h2 class="text-2xl font-mono font-bold mb-2">{selectedDefinition}</h2>
          {#if def.description}
            <div class="text-base-content/70">
              <Markdown content={def.description} />
            </div>
          {/if}
          {#if def.type}
            <div class="mt-2">
              <span class="badge badge-outline">{def.type}</span>
            </div>
          {/if}
        </div>

        <!-- Required fields -->
        {#if def.required && def.required.length > 0}
          <div>
            <h3 class="text-lg font-bold mb-3">Required Fields</h3>
            <div class="flex flex-wrap gap-2">
              {#each def.required as field}
                <span class="badge badge-error">{field}</span>
              {/each}
            </div>
          </div>
        {/if}

        <!-- Properties -->
        {#if def.properties}
          <div>
            <h3 class="text-lg font-bold mb-3">Properties</h3>
            <div class="overflow-x-auto">
              <table class="table table-zebra">
                <thead>
                  <tr>
                    <th>Field</th>
                    <th>Type</th>
                    <th>Required</th>
                    <th>Description</th>
                  </tr>
                </thead>
                <tbody>
                  {#each Object.entries(def.properties) as [propName, propDef]}
                    {@const isRequired = def.required?.includes(propName)}
                    {@const typeStr = formatType(propDef, !isRequired)}
                    {@const typeParts = parseTypeString(typeStr)}
                    <tr>
                      <td>
                        <code class="font-mono text-sm">{propName}</code>
                      </td>
                      <td>
                        <span class="badge badge-sm badge-outline font-mono inline-flex items-center gap-1">
                          {#each typeParts as part}
                            {#if part.isClickable}
                              <button
                                class="hover:text-primary cursor-pointer underline"
                                onclick={() => selectDefinition(part.text)}
                              >
                                {part.text}
                              </button>
                            {:else}
                              <span>{part.text}</span>
                            {/if}
                          {/each}
                        </span>
                      </td>
                      <td>
                        {#if isRequired}
                          <span class="badge badge-sm badge-error">required</span>
                        {:else}
                          <span class="badge badge-sm badge-ghost">optional</span>
                        {/if}
                      </td>
                      <td class="text-sm max-w-md">
                        {#if propDef.description}
                          <InlineMarkdown content={propDef.description} />
                        {:else}
                          -
                        {/if}
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          </div>
        {/if}

        <!-- Enum values -->
        {#if def.enum}
          <div>
            <h3 class="text-lg font-bold mb-3">Allowed Values</h3>
            <div class="flex flex-wrap gap-2">
              {#each def.enum as value}
                <code class="badge badge-lg font-mono">{value}</code>
              {/each}
            </div>
          </div>
        {/if}

        <!-- oneOf/anyOf -->
        {#if def.oneOf || def.anyOf}
          {@const variants = def.oneOf || def.anyOf}
          <div>
            <h3 class="text-lg font-bold mb-3">{def.oneOf ? 'One Of' : 'Any Of'}</h3>
            <div class="space-y-2">
              {#each variants as variant}
                {@const variantType = formatType(variant)}
                {@const variantParts = parseTypeString(variantType)}
                <div class="card bg-base-200">
                  <div class="card-body p-4">
                    <!-- Show enum value if it's a single-value enum (like template types) -->
                    {#if variant.enum && variant.enum.length === 1}
                      <code class="badge badge-outline font-mono">{variant.enum[0]}</code>
                    {:else if variant.type === 'object' && variant.properties}
                      <!-- Show object type with inline properties -->
                      <span class="badge badge-outline font-mono">object</span>
                    {:else}
                      <span class="badge badge-outline font-mono inline-flex items-center gap-1">
                        {#each variantParts as part}
                          {#if part.isClickable}
                            <button
                              class="hover:text-primary cursor-pointer underline"
                              onclick={() => selectDefinition(part.text)}
                            >
                              {part.text}
                            </button>
                          {:else}
                            <span>{part.text}</span>
                          {/if}
                        {/each}
                      </span>
                    {/if}
                    {#if variant.description}
                      <div class="text-sm mt-2">
                        <Markdown content={variant.description} />
                      </div>
                    {/if}

                    <!-- Show object properties inline -->
                    {#if variant.type === 'object' && variant.properties}
                      <div class="mt-3">
                        <table class="table table-xs">
                          <thead>
                            <tr>
                              <th>Field</th>
                              <th>Type</th>
                              <th>Required</th>
                              <th>Description</th>
                            </tr>
                          </thead>
                          <tbody>
                            {#each Object.entries(variant.properties) as [propName, propDef]}
                              {@const isRequired = variant.required?.includes(propName)}
                              {@const typeStr = formatType(propDef, !isRequired)}
                              {@const typeParts = parseTypeString(typeStr)}
                              <tr>
                                <td>
                                  <code class="font-mono text-xs">{propName}</code>
                                </td>
                                <td>
                                  <span class="badge badge-xs badge-outline font-mono inline-flex items-center gap-1">
                                    {#each typeParts as part}
                                      {#if part.isClickable}
                                        <button
                                          class="hover:text-primary cursor-pointer underline"
                                          onclick={() => selectDefinition(part.text)}
                                        >
                                          {part.text}
                                        </button>
                                      {:else}
                                        <span>{part.text}</span>
                                      {/if}
                                    {/each}
                                  </span>
                                  <!-- Show enum values inline if present -->
                                  {#if propDef.enum}
                                    <div class="flex flex-wrap gap-1 mt-1">
                                      {#each propDef.enum as enumVal}
                                        <code class="badge badge-xs">{enumVal}</code>
                                      {/each}
                                    </div>
                                  {/if}
                                </td>
                                <td>
                                  {#if isRequired}
                                    <span class="badge badge-xs badge-error">required</span>
                                  {:else}
                                    <span class="badge badge-xs badge-ghost">optional</span>
                                  {/if}
                                </td>
                                <td class="text-xs">
                                  {#if propDef.description}
                                    <InlineMarkdown content={propDef.description} />
                                  {:else}
                                    -
                                  {/if}
                                </td>
                              </tr>
                            {/each}
                          </tbody>
                        </table>
                      </div>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        <!-- Raw JSON view -->
        <div>
          <h3 class="text-lg font-bold mb-3">Raw JSON</h3>
          <div class="mockup-code bg-base-300">
            <pre class="text-xs overflow-x-auto text-base-content"><code>{JSON.stringify(def, null, 2)}</code></pre>
          </div>
        </div>
      </div>
    {:else}
      <div class="flex items-center justify-center h-full text-base-content/50">
        <div class="text-center">
          <svg xmlns="http://www.w3.org/2000/svg" class="h-16 w-16 mx-auto mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
          <p class="text-lg">Select a type definition to view details</p>
        </div>
      </div>
    {/if}
  </div>
</div>
