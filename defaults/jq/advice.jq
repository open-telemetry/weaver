# Preprocessor for the advice rego policies
{
  # Convert attributes to a set 
  "attributes_set": (
    .semconv_attributes | keys | 
    reduce .[] as $attr ({}; . + {($attr): true})
  ),

  # Convert attributes to a set of deprecated attributes
  "deprecated_attributes_set": (
    .semconv_attributes | 
    to_entries | 
    map(select(.value.deprecated != null)) | 
    map(.key) | 
    reduce .[] as $attr ({}; . + {($attr): true})
  ),
  
  # Convert templates to a set
  "templates_set": (
    .semconv_templates | keys | 
    reduce .[] as $template ({}; . + {($template): true})
  ),
  
  # The schema url of the registry under check. An association leaf omits its
  # provenance when the entity is defined here, so a policy resolves the absence
  # against this.
  "schema_url": .registry.schema_url,

  # The v2 entity definitions, keyed by the schema url of the registry that
  # defines them, and then by entity type or refinement id. That pair is what an
  # association leaf carries, so a policy reads one definition with
  # `data.entities[leaf.provenance.source][leaf.type]` and never has to search by
  # name. `dependencies` holds every registry depended on, keyed by url. A v1
  # registry has none of these paths and gets an empty object.
  "entities": (
    [
      ((.registry.dependencies // {}) | to_entries)[],
      {key: .registry.schema_url, value: .registry}
    ]
    | map(select(.key != null))
    | map({
        key: .key,
        value: (
          ((.value.registry.entities // []) | map({key: .type, value: .}))
          + ((.value.refinements.entities // []) | map({key: .id, value: .}))
          | from_entries
        ),
      })
    | from_entries
  ),

  # Extract all possible namespaces from attributes
  "namespaces_to_check_set": (
    .semconv_attributes | keys | 
    reduce .[] as $attr_name (
      {}; 
      # Get all prefixes up to the full attribute name
      . + reduce range(1; ($attr_name | split(".") | length)) as $i (
        {};
        . + {($attr_name | split(".") | .[0:$i] | join(".")): true}
      )
    )
  )
}