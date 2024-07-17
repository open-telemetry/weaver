def semconv_attributes:
    .groups[]
    | map(select(.id | startswith("registry.")))
    | map(select(.type == "attribute_group"))
    | map(. + {group_id: .id | split(".") | .[1]})
    | sort_by(.group_id);

def semconv_grouped_attributes:
    .groups[]
    | map(select(.id | startswith("registry.")))
    | map(select(.type == "attribute_group"))
    | map(. + {group_id: .id | split(".") | .[1]})
    | sort_by(.group_id)
    | group_by(.group_id);

def semconv_metrics:
    .groups[]
    | map(select(.type == "metric"))
    | map(. + {group_id: .id | split(".") | .[1]})
    | sort_by(.group_id);

def semconv_grouped_metrics:
    .groups[]
    | map(select(.type == "metric"))
    | map(. + {group_id: .id | split(".") | .[1]})
    | sort_by(.group_id)
    | group_by(.group_id);

def semconv_resources:
    .groups[]
    | map(select(.type == "resource"))
    | map(. + {group_id: .id | split(".") | .[1]})
    | sort_by(.group_id);

def semconv_grouped_resources:
    .groups[]
    | map(select(.type == "resource"))
    | map(. + {group_id: .id | split(".") | .[1]})
    | sort_by(.group_id)
    | group_by(.group_id);

def semconv_scopes:
    .groups[]
    | map(select(.type == "scope"))
    | map(. + {group_id: .id | split(".") | .[1]})
    | sort_by(.group_id);

def semconv_grouped_scopes:
    .groups[]
    | map(select(.type == "scope"))
    | map(. + {group_id: .id | split(".") | .[1]})
    | sort_by(.group_id)
    | group_by(.group_id);

def semconv_spans:
    .groups[]
    | map(select(.type == "span"))
    | map(. + {group_id: .id | split(".") | .[1]})
    | sort_by(.group_id);

def semconv_grouped_spans:
    .groups[]
    | map(select(.type == "span"))
    | map(. + {group_id: .id | split(".") | .[1]})
    | sort_by(.group_id)
    | group_by(.group_id);

def semconv_events:
    .groups[]
    | map(select(.type == "event"))
    | map(. + {group_id: .id | split(".") | .[1]})
    | sort_by(.group_id);

def semconv_grouped_events:
    .groups[]
    | map(select(.type == "event"))
    | map(. + {group_id: .id | split(".") | .[1]})
    | sort_by(.group_id)
    | group_by(.group_id);

def semconv_group_by_namespace:
    group_by(.group_id);


