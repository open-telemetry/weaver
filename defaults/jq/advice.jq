# Preprocessor for the advice rego policies
{
  # Convert attributes to a set 
  "attributes_set": (
    .semconv_attributes | keys | 
    reduce .[] as $attr ({}; . + {($attr): true})
  ),
  
  # Convert templates to a set
  "templates_set": (
    .semconv_templates | keys | 
    reduce .[] as $template ({}; . + {($template): true})
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