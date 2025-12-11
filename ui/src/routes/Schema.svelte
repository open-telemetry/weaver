<script>
  let schema = $state(null);
  let error = $state(null);
  let loading = $state(true);
  let selectedDefinition = $state(null);

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

  // Extract definitions from schema
  const definitions = $derived(schema?.definitions ? Object.keys(schema.definitions).sort() : []);

  function selectDefinition(name) {
    selectedDefinition = name;
  }

  function formatType(prop) {
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

    if (Array.isArray(prop.type)) {
      return prop.type.join(' | ');
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
      return prop.anyOf.map(t => t.$ref ? t.$ref.replace('#/definitions/', '') : t.type || 'null').join(' | ');
    }
    if (prop.oneOf) {
      return prop.oneOf.map(t => t.$ref ? t.$ref.replace('#/definitions/', '') : t.type || 'null').join(' | ');
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
    // Check if it contains " | " (union type)
    if (typeStr.includes(' | ')) return false;
    // Otherwise, it's likely a definition reference
    return schema?.definitions && schema.definitions[typeStr] !== undefined;
  }

  // Parse a type string and return clickable parts
  function parseTypeString(typeStr) {
    if (!typeStr) return [];

    // Handle union types (e.g., "Stability | null")
    if (typeStr.includes(' | ')) {
      return typeStr.split(' | ').map(part => ({
        text: part,
        isClickable: isDefinitionRef(part.trim())
      }));
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

    // Single type
    return [{ text: typeStr, isClickable: isDefinitionRef(typeStr) }];
  }

  // Helper to navigate to a definition from a root property
  function selectPropertyType(prop) {
    const propDef = schema.properties[prop];

    // Check for direct $ref
    if (propDef.$ref) {
      const typeName = propDef.$ref.replace('#/definitions/', '');
      selectDefinition(typeName);
      return;
    }

    // Check for allOf with $ref
    if (propDef.allOf?.length > 0 && propDef.allOf[0].$ref) {
      const typeName = propDef.allOf[0].$ref.replace('#/definitions/', '');
      selectDefinition(typeName);
      return;
    }

    // Check for array with $ref
    if (propDef.items?.$ref) {
      const typeName = propDef.items.$ref.replace('#/definitions/', '');
      selectDefinition(typeName);
      return;
    }

    // Check for anyOf with single $ref
    if (propDef.anyOf?.length === 1 && propDef.anyOf[0].$ref) {
      const typeName = propDef.anyOf[0].$ref.replace('#/definitions/', '');
      selectDefinition(typeName);
      return;
    }

    // Check for anyOf with $ref (skip null types)
    if (propDef.anyOf) {
      const refItem = propDef.anyOf.find(item => item.$ref);
      if (refItem) {
        const typeName = refItem.$ref.replace('#/definitions/', '');
        selectDefinition(typeName);
        return;
      }
    }
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
        <div class="card bg-base-200 sticky top-0 z-10">
          <div class="card-body p-4">
            <h1 class="font-bold text-xl">{schema.title || 'Schema'}</h1>
            <p class="text-sm text-base-content/70">{schema.description || ''}</p>
          </div>
        </div>

        <!-- Top-level properties -->
        <div>
          <h3 class="font-bold text-sm mb-2 text-base-content/70">ROOT PROPERTIES</h3>
          <div class="space-y-1">
            {#each Object.keys(schema.properties || {}) as prop}
              <button
                class="w-full text-left px-3 py-2 bg-base-200 hover:bg-base-300 rounded text-sm transition-colors cursor-pointer"
                onclick={() => selectPropertyType(prop)}
              >
                <code class="font-mono">{prop}</code>
                {#if schema.properties[prop].description}
                  <p class="text-xs text-base-content/60 mt-1">{schema.properties[prop].description}</p>
                {/if}
              </button>
            {/each}
          </div>
        </div>

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
    {#if selectedDefinition && schema?.definitions[selectedDefinition]}
      {@const def = schema.definitions[selectedDefinition]}
      <div class="space-y-6">
        <!-- Header -->
        <div>
          <h2 class="text-2xl font-mono font-bold mb-2">{selectedDefinition}</h2>
          {#if def.description}
            <p class="text-base-content/70">{def.description}</p>
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
                    {@const typeStr = formatType(propDef)}
                    {@const typeParts = parseTypeString(typeStr)}
                    <tr>
                      <td>
                        <code class="font-mono text-sm">{propName}</code>
                      </td>
                      <td>
                        <span class="badge badge-sm badge-outline font-mono inline-flex items-center gap-1">
                          {#each typeParts as part, i}
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
                            {#if i < typeParts.length - 1 && typeStr.includes(' | ')}
                              <span class="mx-1">|</span>
                            {/if}
                          {/each}
                        </span>
                      </td>
                      <td>
                        {#if def.required?.includes(propName)}
                          <span class="badge badge-sm badge-error">required</span>
                        {:else}
                          <span class="badge badge-sm badge-ghost">optional</span>
                        {/if}
                      </td>
                      <td class="text-sm max-w-md">{propDef.description || '-'}</td>
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
                    {#if variant.description}
                      <p class="text-sm mt-2">{variant.description}</p>
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
