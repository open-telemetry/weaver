##########################
# Generic Signal Functions
##########################

# Groups by the root namespace and assigns the children to the provided signal and sorts them by id.
# $signal is the type of signal to group.
def semconv_group_by_root_namespace($signal):
    group_by(.root_namespace)
    | map({
        group_namespace: .[0].root_namespace,
        root_namespace: .[0].root_namespace,
        ($signal): . | sort_by(.id)
      });

# Groups by the full namespace and assigns the children to the provided signal and sorts them by id.
# $signal is the type of signal to group.
def semconv_group_by_full_namespace($signal):
    group_by(.full_namespace)
    | map({
        group_namespace: .[0].full_namespace,
        full_namespace: .[0].full_namespace,
        ($signal): . | sort_by(.id)
      });

# Extracts and processes semantic convention signals based on provided options.
# The root and full namespace excludes the first "xxxxx." from the "id" (like
# metric.xxx and registry.xxx)
# $signal is the type of signal to process.
# $options is an object that can contain:
# - exclude_stability: a list of stability statuses to exclude.
# - exclude_deprecated: a boolean to exclude deprecated signals.
# - exclude_root_namespace: a list of root namespaces to exclude.
# - exclude_full_namespace: a list of full namespaces to exclude.
def semconv_signal($signal; $options):
    .groups
    | map(select(.type == $signal))
    | if ($options | has("exclude_stability")) then
        map(select(.stability as $st | $options.exclude_stability | index($st) | not))
      else
        .
      end
    | if ($options | has("exclude_deprecated") and $options.exclude_deprecated == true) then
        map(select(.id | endswith(".deprecated") | not))
      else
        .
      end
    | map(. + {
        root_namespace: (if .id | index(".") then .id | split(".") | .[1] else "other" end),
        full_namespace: (if .id | index(".") then (.id | split(".") | .[1:-1] | join(".")) else "other" end)
      })
    | if ($options | has("exclude_root_namespace")) then
        map(select(.root_namespace as $st | $options.exclude_root_namespace | index($st) | not))
      else
        .
      end
    | if ($options | has("exclude_full_namespace")) then
        map(select(.full_namespace as $st | $options.exclude_full_namespace | index($st) | not))
      else
        .
      end
    | sort_by(.root_namespace, .id);

# Processes semantic convention signals based on provided options.
# The root and full namespace includes the first "xxxxx." from the "id"
# This is different from semconv_signal which excludes it (for metrics.metric_name)
# $signal is the type of signal to process.
# $options is an object that can contain:
# - exclude_stability: a list of stability statuses to exclude.
# - exclude_deprecated: a boolean to exclude deprecated signals.
# - exclude_root_namespace: a list of root namespaces to exclude.
# - exclude_full_namespace: a list of full namespaces to exclude.
def semconv_signal_all($signal; $options):
    .groups
    | map(select(.type == $signal))
    | if ($options | has("exclude_stability")) then
        map(select(.stability as $st | $options.exclude_stability | index($st) | not))
      else
        .
      end
    | if ($options | has("exclude_deprecated") and $options.exclude_deprecated == true) then
        map(select(.id | endswith(".deprecated") | not))
      else
        .
      end
    | map(. + {
        root_namespace: (if .id | index(".") then .id | split(".")[0] else "other" end),
        full_namespace: (if .id | index(".") then (.id | split(".") | .[0:-1] | join(".")) else "other" end)
      })
    | if ($options | has("exclude_root_namespace")) then
        map(select(.root_namespace as $st | $options.exclude_root_namespace | index($st) | not))
      else
        .
      end
    | if ($options | has("exclude_full_namespace")) then
        map(select(.full_namespace as $st | $options.exclude_full_namespace | index($st) | not))
      else
        .
      end
    | sort_by(.full_namespace, .id);

#####################
# Attribute functions
#####################

# Extracts and processes semantic convention attributes based on provided options.
# $options is an object that can contain:
# - exclude_stability: a list of stability statuses to exclude.
# - exclude_deprecated: a boolean to exclude deprecated attributes.
# - exclude_root_namespace: a list of root namespaces to exclude.
# - exclude_full_namespace: a list of full namespaces to exclude.
def semconv_attributes($options):
    .groups
    | map(select(.type == "attribute_group" and (.id | startswith("registry."))))
    | map(.attributes) | add
    | if ($options | has("exclude_stability")) then
        map(select(.stability as $st | $options.exclude_stability | index($st) | not))
      else
        .
      end
    | if ($options | has("exclude_deprecated") and $options.exclude_deprecated == true) then
        map(select(has("deprecated") | not))
      else
        .
      end
    | map(. + {
        root_namespace: (if .name | index(".") then .name | split(".")[0] else "other" end),
        full_namespace: (if .name | index(".") then (.name | split(".") | .[0:-1] | join(".")) else "other" end)
      })
    | if ($options | has("exclude_root_namespace")) then
        map(select(.root_namespace as $st | $options.exclude_root_namespace | index($st) | not))
      else
        .
      end
    | if ($options | has("exclude_full_namespace")) then
        map(select(.full_namespace as $st | $options.exclude_full_namespace | index($st) | not))
      else
        .
      end
    | sort_by(.root_namespace, .name);

# Convenience function to extract all attributes without any filtering options.
def semconv_attributes: semconv_attributes({});

# Groups the processed attributes by their root namespace based on provided options.
# $options is an object that can contain:
# - exclude_stability: a list of stability statuses to exclude.
# - exclude_deprecated: a boolean to exclude deprecated attributes.
# - exclude_root_namespace: a list of root namespaces to exclude.
# - exclude_full_namespace: a list of full namespaces to exclude.
def semconv_grouped_attributes($options):
    semconv_attributes($options)
    | semconv_group_by_root_namespace("attributes");

# Convenience function to group all attributes by their root namespace without
# any filtering options.
def semconv_grouped_attributes: semconv_grouped_attributes({});

##################
# Metric Functions
##################
# Groups the metrics by their root namespace and sorts metrics by metric_name.
def semconv_group_metrics_by_root_namespace:
    group_by(.root_namespace)
    | map({
        group_namespace: .[0].root_namespace,
        root_namespace: .[0].root_namespace,
        metrics: . | sort_by(.metric_name)
      });

# Extracts and processes semantic convention metrics based on provided options.
# $options is an object that can contain:
# - exclude_stability: a list of stability statuses to exclude.
# - exclude_deprecated: a boolean to exclude deprecated metrics.
# - exclude_root_namespace: a list of root namespaces to exclude.
# - exclude_full_namespace: a list of full namespaces to exclude.
def semconv_metrics($options): semconv_signal("metric"; $options);

# Convenience function to extract all metrics without any filtering options.
def semconv_metrics: semconv_metrics({});

# Groups the processed metrics by their root namespace based on provided options.
# $options is an object that can contain:
# - exclude_stability: a list of stability statuses to exclude.
# - exclude_deprecated: a boolean to exclude deprecated metrics.
# - exclude_root_namespace: a list of root namespaces to exclude.
# - exclude_full_namespace: a list of full namespaces to exclude.
def semconv_grouped_metrics($options):
  semconv_metrics($options) 
  | semconv_group_metrics_by_root_namespace;

# Convenience function to group all metrics by their root namespace without any filtering options.
def semconv_grouped_metrics: semconv_grouped_metrics({});

#################
# Event Functions
#################

# Extracts and processes semantic convention events based on provided options.
# $options is an object that can contain:
# - exclude_stability: a list of stability statuses to exclude.
# - exclude_deprecated: a boolean to exclude deprecated events.
# - exclude_root_namespace: a list of root namespaces to exclude.
# - exclude_full_namespace: a list of full namespaces to exclude.
def semconv_events($options): semconv_signal_all("event"; $options);

# Convenience function to extract all events without any filtering options.
def semconv_events: semconv_events({});

# Groups the processed events by their root namespace based on provided options.
# $options is an object that can contain:
# - exclude_stability: a list of stability statuses to exclude.
# - exclude_deprecated: a boolean to exclude deprecated events.
# - exclude_root_namespace: a list of root namespaces to exclude.
# - exclude_full_namespace: a list of full namespaces to exclude.
def semconv_grouped_events($options): 
  semconv_events($options)
  | semconv_group_by_full_namespace("events");

# Convenience function to group all events by their root namespace without any filtering options.
def semconv_grouped_events: semconv_grouped_events({});
