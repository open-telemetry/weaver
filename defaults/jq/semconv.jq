# Groups the attributes by their root namespace and sorts them by name.
def semconv_group_attributes_by_root_namespace:
    group_by(.root_namespace)
    | map({ root_namespace: .[0].root_namespace, attributes: . | sort_by(.name) });

# Expands stability array that previously used "experimental" for equivalent new strings.
def expand_stability($stability):
  if ($stability | index("experimental")) then
    $stability + [ "development", "alpha", "beta", "release_candidate", "", null ]
  else
    $stability
  end;

# Filters the input based on the stability status and stability configuration options.
# $options is an object that can contain:
# - stable_only: a boolean to exclude all non-stable attributes.
# - exclude_stability: a list of stability statuses to exclude. Use `stable_only` to exclude all non-stable attributes instead.
def stability_filter($options):
    if $options.stable_only then
        map(select(.stability == "stable"))
    else
        .
    end
    | if $options | has("exclude_stability") then
        map(select(.stability as $st | expand_stability($options.exclude_stability) | index($st) | not))
    else
        .
    end;

# Expands signal argument into a filter on groups.
def signal_filter($signal):
  if ($signal | index("resource")) then
    map(select(.type == "entity"))
  else
    map(select(.type == $signal))
  end;

# Filters out attributes based on code generation annotations.
# $options is an object that can contain:
# - ignore_code_generation_annotations: a boolean to ignore code generation annotations.
def code_generation_exclude_filter($options):
    if ($options | has("ignore_code_generation_annotations")) then
        .
    else
        # null coalescence is not supported in jaq (but supported in jq)
        map(select(
            .annotations == null 
            or .annotations.code_generation == null 
            or .annotations.code_generation.exclude == null 
            or .annotations.code_generation.exclude == false
        ))
    end;

#####################
# Attribute functions
#####################

# Extracts and processes semantic convention attributes based on provided options.
# $options is an object that can contain:
# - exclude_deprecated: a boolean to exclude deprecated attributes.
# - exclude_root_namespace: a list of root namespaces to exclude.
# - stable_only: a boolean to exclude all non-stable attributes.
# - exclude_stability: a list of stability statuses to exclude. Use `stable_only` to exclude all non-stable attributes instead.
# - ignore_code_generation_annotations: a boolean to ignore code generation annotations.
def semconv_attributes($options):
    .groups
    | map(select(.type == "attribute_group" and (.id | startswith("registry."))))
    | map(.attributes) | add
    | stability_filter($options)
    | code_generation_exclude_filter($options)
    | if ($options | has("exclude_deprecated") and $options.exclude_deprecated == true) then
        map(select(has("deprecated") | not))
      else
        .
      end
    | map(. + {root_namespace: (if .name | index(".") then .name | split(".")[0] else "other" end)})
    | if ($options | has("exclude_root_namespace")) then
        map(select(.root_namespace as $st | $options.exclude_root_namespace | index($st) | not))
      else
        .
      end
    | sort_by(.root_namespace, .name);

# Convenience function to extract all attributes without any filtering options.
def semconv_attributes: semconv_attributes({});

# Groups the processed attributes by their root namespace based on provided options.
# $options is an object that can contain:
# - stable_only: a boolean to exclude all non-stable attributes.
# - exclude_deprecated: a boolean to exclude deprecated attributes.
# - exclude_root_namespace: a list of root namespaces to exclude.
# - exclude_stability: a list of stability statuses to exclude. Use `stable_only` to exclude all non-stable attributes instead.
def semconv_grouped_attributes($options):
    semconv_attributes($options)
    | semconv_group_attributes_by_root_namespace;

# Convenience function to group all attributes by their root namespace without
# any filtering options.
def semconv_grouped_attributes: semconv_grouped_attributes({});

# Generic Signal Functions

# Extracts and processes semantic convention signals based on provided options.
# $signal is the type of signal to process.
# $options is an object that can contain:
# - stable_only: a boolean to exclude all non-stable signals.
# - exclude_deprecated: a boolean to exclude deprecated signals.
# - exclude_root_namespace: a list of root namespaces to exclude.
# - exclude_stability: a list of stability statuses to exclude. Use `stable_only` to exclude all non-stable signals instead.
def semconv_signal($signal; $options):
    .groups
    | signal_filter($signal)
    | stability_filter($options)
    | if ($options | has("exclude_deprecated") and $options.exclude_deprecated == true) then
        map(select(.id | endswith(".deprecated") | not))
      else
        .
      end
    | map(. + {root_namespace: (if .id | index(".") then .id | split(".") | .[1] else "other" end)})
    | if ($options | has("exclude_root_namespace")) then
        map(select(.root_namespace as $st | $options.exclude_root_namespace | index($st) | not))
      else
        .
      end
    | sort_by(.root_namespace);

# Metric Functions
# Groups the metrics by their root namespace and sorts metrics by metric_name.
def semconv_group_metrics_by_root_namespace:
    group_by(.root_namespace)
    | map({ root_namespace: .[0].root_namespace, metrics: . | sort_by(.metric_name) });

# Extracts and processes semantic convention metrics based on provided options.
# $options is an object that can contain:
# - stable_only: a boolean to exclude all non-stable metrics.
# - exclude_deprecated: a boolean to exclude deprecated metrics.
# - exclude_root_namespace: a list of root namespaces to exclude.
# - exclude_stability: a list of stability statuses to exclude. Use `stable_only` to exclude all non-stable metrics instead.
def semconv_metrics($options): semconv_signal("metric"; $options) | sort_by(.metric_name);

# Convenience function to extract all metrics without any filtering options.
def semconv_metrics: semconv_metrics({});

# Groups the processed metrics by their root namespace based on provided options.
# $options is an object that can contain:
# - stable_only: a boolean to exclude all non-stable metrics.
# - exclude_deprecated: a boolean to exclude deprecated metrics.
# - exclude_root_namespace: a list of root namespaces to exclude.
# - exclude_stability: a list of stability statuses to exclude. Use `stable_only` to exclude all non-stable metrics instead.
def semconv_grouped_metrics($options): semconv_metrics($options) | semconv_group_metrics_by_root_namespace;

# Convenience function to group all metrics by their root namespace without any filtering options.
def semconv_grouped_metrics: semconv_grouped_metrics({});
