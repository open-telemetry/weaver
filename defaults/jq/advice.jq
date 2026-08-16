# Preprocessor for the advice rego policies
#
# The live checker keeps the registry under check apart from its dependency
# closure, because the attribute lookup searches the registry first. The policies
# only ask whether a definition exists anywhere, so the two are merged here.
# The registry under check is the right operand of `+`, so its definition wins
# on a key that both sides hold. That matches the lookup, which searches the
# registry under check before any dependency.
. as $root
| (($root.dependency_attributes // {}) + $root.semconv_attributes) as $attributes
| (($root.dependency_templates // {}) + $root.semconv_templates) as $templates
| {
  # Convert attributes to a set 
  "attributes_set": (
    $attributes | keys | 
    reduce .[] as $attr ({}; . + {($attr): true})
  ),

  # Convert attributes to a set of deprecated attributes
  "deprecated_attributes_set": (
    $attributes | 
    to_entries | 
    map(select(.value.deprecated != null)) | 
    map(.key) | 
    reduce .[] as $attr ({}; . + {($attr): true})
  ),
  
  # Convert templates to a set
  "templates_set": (
    $templates | keys | 
    reduce .[] as $template ({}; . + {($template): true})
  ),
  
  # Extract all possible namespaces from attributes
  "namespaces_to_check_set": (
    $attributes | keys | 
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
