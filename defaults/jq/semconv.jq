def semconv_group_attributes_by_namespace:
    group_by(.namespace)
    | map({ namespace: .[0].namespace, attributes: . | sort_by(.name) });

#####################
# Attribute functions
#####################
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
    | map(. + {namespace: (if .name | index(".") then .name | split(".")[0] else "other" end)})
    | if ($options | has("exclude_namespace")) then
        map(select(.namespace as $st | $options.exclude_namespace | index($st) | not))
      else
        .
      end
    | sort_by(.namespace, .name);

def semconv_attributes: semconv_attributes({});

def semconv_grouped_attributes($options):
    semconv_attributes($options)
    | semconv_group_attributes_by_namespace;

def semconv_grouped_attributes: semconv_grouped_attributes({});

# Generic Signal Functions
def semconv_group_signals_by_namespace($signal):
    group_by(.namespace)
    | map({ namespace: .[0].namespace, ($signal): . | sort_by(.name) });

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
    | map(. + {namespace: .id | split(".") | .[1]})
    | if ($options | has("exclude_namespace")) then
        map(select(.namespace as $st | $options.exclude_namespace | index($st) | not))
      else
        .
      end
    | sort_by(.namespace);

# Metric Functions
def semconv_group_metrics_by_namespace: semconv_group_signals_by_namespace("metrics");
def semconv_metrics($options): semconv_signal("metric"; $options);
def semconv_metrics: semconv_metrics({});

def semconv_grouped_metrics($options): semconv_metrics($options) | semconv_group_metrics_by_namespace;
def semconv_grouped_metrics: semconv_grouped_metrics({});

# Resource functions
def semconv_group_resources_by_namespace: semconv_group_signals_by_namespace("resources");
def semconv_resources($options): semconv_signal("resource"; $options);
def semconv_resources: semconv_resources({});

def semconv_grouped_resources($options): semconv_resources($options) | semconv_group_resources_by_namespace;
def semconv_grouped_resources: semconv_grouped_resources({});

# Scope functions
def semconv_group_scopes_by_namespace: semconv_group_signals_by_namespace("scopes");
def semconv_scopes($options): semconv_signal("scope"; $options);
def semconv_scopes: semconv_scopes({});

def semconv_grouped_scopes($options): semconv_scopes($options) | semconv_group_scopes_by_namespace;
def semconv_grouped_scopes: semconv_grouped_scopes({});

# Span functions
def semconv_group_spans_by_namespace: semconv_group_signals_by_namespace("spans");
def semconv_spans($options): semconv_signal("span"; $options);
def semconv_spans: semconv_spans({});

def semconv_grouped_spans($options): semconv_spans($options) | semconv_group_spans_by_namespace;
def semconv_grouped_spans: semconv_grouped_spans({});

# Event functions
def semconv_group_events_by_namespace: semconv_group_signals_by_namespace("events");
def semconv_events($options): semconv_signal("event"; $options);
def semconv_events: semconv_events({});

def semconv_grouped_events($options): semconv_events($options) | semconv_group_events_by_namespace;
def semconv_grouped_events: semconv_grouped_events({});
