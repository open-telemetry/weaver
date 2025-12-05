// copied from https://github.com/open-telemetry/opentelemetry-rust/blob/main/opentelemetry-prometheus/src/utils.rs
// with minor modifications
// - using String instead of Cow
// - making functions pub(crate)
// - adding get_suffixes function that adds _total for counters
// - returning multiple possible names in get_names

use itertools::Itertools;

const NON_APPLICABLE_ON_PER_UNIT: [&str; 8] = ["1", "d", "h", "min", "s", "ms", "us", "ns"];

pub(crate) fn get_names(name: &str, unit: &str, instrument: &str) -> Vec<String> {
    // all possible names when using https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/sdk_exporters/prometheus.md#configuration
    [
        name.to_owned(),                                       // NoTranslation
        sanitize_name(name),                                   // UnderscoreEscapingWithoutSuffixes
        with_suffixes(name, unit, instrument),                 // NoUTF8EscapingWithSuffixes
        with_suffixes(&sanitize_name(name), unit, instrument), // UnderscoreEscapingWithSuffixes
    ]
    .iter()
    .unique()
    .flat_map(|n| {
        if instrument == "histogram" {
            vec![
                n.to_owned(), // native histogram name
                format!("{n}_bucket"),
                format!("{n}_count"),
                format!("{n}_sum"),
            ]
        } else if instrument == "summary" {
            vec![
                n.to_owned(), // for streaming quantiles
                format!("{n}_count"),
                format!("{n}_sum"),
            ]
        } else {
            vec![n.to_owned()]
        }
    })
    .collect()
}

fn with_suffixes(name: &str, unit: &str, instrument: &str) -> String {
    get_suffixes(unit, instrument)
        .into_iter()
        .fold(name.to_owned(), |acc, suffix| format!("{acc}_{suffix}"))
}

pub(crate) fn get_suffixes(unit: &str, instrument: &str) -> Vec<String> {
    let mut suffixes = Vec::new();

    // get unit suffixes
    if let Some(unit_suffix) = get_unit_suffixes(unit) {
        suffixes.push(unit_suffix);
    }

    // add _total for counters
    if instrument == "counter" {
        suffixes.push("total".to_owned());
    }

    suffixes
}

pub(crate) fn get_unit_suffixes(unit: &str) -> Option<String> {
    // no unit return early
    if unit.is_empty() {
        return None;
    }

    // direct match with known units
    if let Some(matched) = get_prom_units(unit) {
        return Some(matched.to_owned());
    }

    // converting foo/bar to foo_per_bar
    // split the string by the first '/'
    // if the first part is empty, we just return the second part if it's a match with known per unit
    // e.g
    // "test/y" => "per_year"
    // "km/s" => "kilometers_per_second"
    if let Some((first, second)) = unit.split_once('/') {
        return match (
            NON_APPLICABLE_ON_PER_UNIT.contains(&first),
            get_prom_units(first),
            get_prom_per_unit(second),
        ) {
            (true, _, Some(second_part)) | (false, None, Some(second_part)) => {
                Some(format!("per_{second_part}"))
            }
            (false, Some(first_part), Some(second_part)) => {
                Some(format!("{first_part}_per_{second_part}"))
            }
            _ => None,
        };
    }

    // Unmatched units and annotations are ignored
    // e.g. "{request}"
    None
}

pub(crate) fn get_prom_units(unit: &str) -> Option<&'static str> {
    match unit {
        // Time
        "d" => Some("days"),
        "h" => Some("hours"),
        "min" => Some("minutes"),
        "s" => Some("seconds"),
        "ms" => Some("milliseconds"),
        "us" => Some("microseconds"),
        "ns" => Some("nanoseconds"),

        // Bytes
        "By" => Some("bytes"),
        "KiBy" => Some("kibibytes"),
        "MiBy" => Some("mebibytes"),
        "GiBy" => Some("gibibytes"),
        "TiBy" => Some("tibibytes"),
        "KBy" => Some("kilobytes"),
        "MBy" => Some("megabytes"),
        "GBy" => Some("gigabytes"),
        "TBy" => Some("terabytes"),
        "B" => Some("bytes"),
        "KB" => Some("kilobytes"),
        "MB" => Some("megabytes"),
        "GB" => Some("gigabytes"),
        "TB" => Some("terabytes"),

        // SI
        "m" => Some("meters"),
        "V" => Some("volts"),
        "A" => Some("amperes"),
        "J" => Some("joules"),
        "W" => Some("watts"),
        "g" => Some("grams"),

        // Misc
        "Cel" => Some("celsius"),
        "Hz" => Some("hertz"),
        "1" => Some("ratio"),
        "%" => Some("percent"),
        _ => None,
    }
}

fn get_prom_per_unit(unit: &str) -> Option<&'static str> {
    match unit {
        "s" => Some("second"),
        "m" => Some("minute"),
        "h" => Some("hour"),
        "d" => Some("day"),
        "w" => Some("week"),
        "mo" => Some("month"),
        "y" => Some("year"),
        _ => None,
    }
}

#[allow(clippy::ptr_arg)]
pub(crate) fn sanitize_name(s: &str) -> String {
    // prefix chars to add in case name starts with number
    let mut prefix = "";

    // Find first invalid char
    if let Some((replace_idx, _)) = s.char_indices().find(|(i, c)| {
        if *i == 0 && c.is_ascii_digit() {
            // first char is number, add prefix and replace reset of chars
            prefix = "_";
            true
        } else {
            // keep checking
            !c.is_alphanumeric() && *c != '_' && *c != ':'
        }
    }) {
        // up to `replace_idx` have been validated, convert the rest
        let (valid, rest) = s.split_at(replace_idx);
        prefix
            .chars()
            .chain(valid.chars())
            .chain(rest.chars().map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == ':' {
                    c
                } else {
                    '_'
                }
            }))
            .collect()
    } else {
        s.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_sanitization() {
        let tests = vec![
            ("name€_with_3_width_rune.", "name__with_3_width_rune_"),
            ("`", "_"),
            (
                r##"! "#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWKYZ[]\^_abcdefghijklmnopqrstuvwkyz{|}~"##,
                "________________0123456789:______ABCDEFGHIJKLMNOPQRSTUVWKYZ_____abcdefghijklmnopqrstuvwkyz____",
            ),

            ("Avalid_23name", "Avalid_23name"),
            ("_Avalid_23name", "_Avalid_23name"),
            ("1valid_23name", "_1valid_23name"),
            ("avalid_23name", "avalid_23name"),
            ("Ava:lid_23name", "Ava:lid_23name"),
            ("a lid_23name", "a_lid_23name"),
            (":leading_colon", ":leading_colon"),
            ("colon:in:the:middle", "colon:in:the:middle"),
            ("", ""),
        ];

        for (input, want) in tests {
            assert_eq!(want, sanitize_name(input), "input: {input}");
        }
    }

    #[test]
    fn test_get_unit_suffixes() {
        let test_cases = vec![
            // Direct match
            ("g", Some("grams")),
            ("s", Some("seconds")),
            // Per unit
            ("test/y", Some("per_year")),
            ("1/y", Some("per_year")),
            ("m/s", Some("meters_per_second")),
            // No match
            ("invalid", None),
            ("invalid/invalid", None),
            ("seconds", None),
            ("", None),
            // annotations
            ("{request}", None),
        ];
        for (unit, expected_suffix) in test_cases {
            assert_eq!(get_unit_suffixes(unit), expected_suffix.map(String::from));
        }
    }

    #[test]
    fn test_get_names() {
        let test_cases = vec![
            // Basic counter with unit (ratio)
            // unique names: ["http_requests", "http_requests_ratio_total"]
            (
                "http_requests",
                "1",
                "counter",
                vec!["http_requests", "http_requests_ratio_total"],
            ),
            // Counter without unit
            // unique names: ["http_requests", "http_requests_total"]
            (
                "http_requests",
                "",
                "counter",
                vec!["http_requests", "http_requests_total"],
            ),
            // Histogram with unit
            // unique names: ["http_request_duration", "http_request_duration_seconds"]
            // each expands to 4 names (base, _bucket, _count, _sum)
            (
                "http_request_duration",
                "s",
                "histogram",
                vec![
                    "http_request_duration",
                    "http_request_duration_bucket",
                    "http_request_duration_count",
                    "http_request_duration_sum",
                    "http_request_duration_seconds",
                    "http_request_duration_seconds_bucket",
                    "http_request_duration_seconds_count",
                    "http_request_duration_seconds_sum",
                ],
            ),
            // Summary without unit
            // unique names: ["rpc_duration"]
            // expands to 3 names (base, _count, _sum)
            (
                "rpc_duration",
                "",
                "summary",
                vec!["rpc_duration", "rpc_duration_count", "rpc_duration_sum"],
            ),
            // Gauge with bytes unit
            // unique names: ["memory_usage", "memory_usage_bytes"]
            (
                "memory_usage",
                "By",
                "gauge",
                vec!["memory_usage", "memory_usage_bytes"],
            ),
            // Counter with per-unit
            // unique names: ["requests", "requests_per_second_total"]
            (
                "requests",
                "1/s",
                "counter",
                vec!["requests", "requests_per_second_total"],
            ),
            // Gauge with special characters (dot) and unit
            // unique names: ["http.requests", "http_requests", "http.requests_milliseconds", "http_requests_milliseconds"]
            (
                "http.requests",
                "ms",
                "gauge",
                vec![
                    "http.requests",
                    "http_requests",
                    "http.requests_milliseconds",
                    "http_requests_milliseconds",
                ],
            ),
            // Counter with dot in name and unit
            // name.to_owned() = "http.server.requests"
            // sanitize_name(name) = "http_server_requests"
            // with_suffixes(name, "1", "counter") = "http.server.requests_ratio_total"
            // with_suffixes(sanitize, "1", "counter") = "http_server_requests_ratio_total"
            // unique: ["http.server.requests", "http_server_requests", "http.server.requests_ratio_total", "http_server_requests_ratio_total"]
            (
                "http.server.requests",
                "1",
                "counter",
                vec![
                    "http.server.requests",
                    "http_server_requests",
                    "http.server.requests_ratio_total",
                    "http_server_requests_ratio_total",
                ],
            ),
            // Histogram with dot in name
            // name.to_owned() = "http.request.duration"
            // sanitize_name(name) = "http_request_duration"
            // with_suffixes(name, "s", "histogram") = "http.request.duration_seconds"
            // with_suffixes(sanitize, "s", "histogram") = "http_request_duration_seconds"
            // unique: ["http.request.duration", "http_request_duration", "http.request.duration_seconds", "http_request_duration_seconds"]
            // each expands to 4 names
            (
                "http.request.duration",
                "s",
                "histogram",
                vec![
                    "http.request.duration",
                    "http.request.duration_bucket",
                    "http.request.duration_count",
                    "http.request.duration_sum",
                    "http_request_duration",
                    "http_request_duration_bucket",
                    "http_request_duration_count",
                    "http_request_duration_sum",
                    "http.request.duration_seconds",
                    "http.request.duration_seconds_bucket",
                    "http.request.duration_seconds_count",
                    "http.request.duration_seconds_sum",
                    "http_request_duration_seconds",
                    "http_request_duration_seconds_bucket",
                    "http_request_duration_seconds_count",
                    "http_request_duration_seconds_sum",
                ],
            ),
        ];

        for (name, unit, instrument, expected) in test_cases {
            let result = get_names(name, unit, instrument);
            assert_eq!(
                result, expected,
                "Failed for name={}, unit={}, instrument={}",
                name, unit, instrument
            );
        }
    }
}
