def semconv_attributes:
    .groups[]
    | map(select(.id | startswith("registry.")))
    | map(select(.type == "attribute_group"))
    | map(. + {group_id: .id | split(".") | .[1]});

def semconv_metrics:
    .groups[]
    | map(select(.type == "metric"))
    | map(. + {group_id: .id | split(".") | .[1]});

def semconv_resources:
    .groups[]
    | map(select(.type == "resource"))
    | map(. + {group_id: .id | split(".") | .[1]});

def semconv_scopes:
    .groups[]
    | map(select(.type == "scope"))
    | map(. + {group_id: .id | split(".") | .[1]});

def semconv_spans:
    .groups[]
    | map(select(.type == "span"))
    | map(. + {group_id: .id | split(".") | .[1]});

def semconv_events:
    .groups[]
    | map(select(.type == "event"))
    | map(. + {group_id: .id | split(".") | .[1]});

def semconv_group_by_namespace:
    sort_by(.group_id)
    | group_by(.group_id);


