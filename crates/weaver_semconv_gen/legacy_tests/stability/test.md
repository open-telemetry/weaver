<!-- semconv test -->
| Attribute  | Type | Description  | Examples  | [Requirement Level](https://opentelemetry.io/docs/specs/semconv/general/attribute-requirement-level/) | Stability |
|---|---|---|---|---|---|
| test.deprecated_experimental_attr | boolean |  |  | `Required` | ![Deprecated](https://img.shields.io/badge/-deprecated-red)<br>Removed. |
| test.deprecated_stable_attr | boolean |  |  | `Required` | ![Deprecated](https://img.shields.io/badge/-deprecated-red)<br>Removed. |
| test.exp_attr | boolean |  |  | `Required` | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| test.stable_attr | boolean |  |  | `Required` | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| test.stable_enum_attr | string |  | `one`; `two`; `three` | `Recommended` | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |

`test.stable_enum_attr` has the following list of well-known values. If one of them applies, then the respective value MUST be used; otherwise, a custom value MAY be used.

| Value  | Description | Stability |
|---|---|---|
| `one` | member one | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| `two` | member two | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `three` | member three | ![Deprecated](https://img.shields.io/badge/-deprecated-red)<br>Removed. |
| `four` | member four | ![Deprecated](https://img.shields.io/badge/-deprecated-red)<br>Removed. |
<!-- endsemconv -->

<!-- semconv ref_test -->
| Attribute  | Type | Description  | Examples  | [Requirement Level](https://opentelemetry.io/docs/specs/semconv/general/attribute-requirement-level/) | Stability |
|---|---|---|---|---|---|
| test.deprecated_experimental_attr | boolean |  |  | `Required` | ![Deprecated](https://img.shields.io/badge/-deprecated-red)<br>Removed. |
| test.deprecated_stable_attr | boolean |  |  | `Required` | ![Deprecated](https://img.shields.io/badge/-deprecated-red)<br>Removed. |
| test.exp_attr | boolean |  |  | `Required` | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| test.stable_attr | boolean |  |  | `Required` | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| test.stable_enum_attr | string |  | `one`; `two`; `three` | `Recommended` | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |

`test.stable_enum_attr` has the following list of well-known values. If one of them applies, then the respective value MUST be used; otherwise, a custom value MAY be used.

| Value  | Description | Stability |
|---|---|---|
| `one` | member one | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| `two` | member two | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `three` | member three | ![Deprecated](https://img.shields.io/badge/-deprecated-red)<br>Removed. |
| `four` | member four | ![Deprecated](https://img.shields.io/badge/-deprecated-red)<br>Removed. |
<!-- endsemconv -->

<!-- semconv extends_test(full) -->
| Attribute  | Type | Description  | Examples  | [Requirement Level](https://opentelemetry.io/docs/specs/semconv/general/attribute-requirement-level/) | Stability |
|---|---|---|---|---|---|
| test.deprecated_experimental_attr | boolean |  |  | `Required` | ![Deprecated](https://img.shields.io/badge/-deprecated-red)<br>Removed. |
| test.deprecated_stable_attr | boolean |  |  | `Required` | ![Deprecated](https://img.shields.io/badge/-deprecated-red)<br>Removed. |
| test.exp_attr | boolean |  |  | `Required` | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| test.stable_attr | boolean |  |  | `Required` | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| test.stable_enum_attr | string |  | `one`; `two`; `three` | `Recommended` | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |

`test.stable_enum_attr` has the following list of well-known values. If one of them applies, then the respective value MUST be used; otherwise, a custom value MAY be used.

| Value  | Description | Stability |
|---|---|---|
| `one` | member one | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| `two` | member two | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `three` | member three | ![Deprecated](https://img.shields.io/badge/-deprecated-red)<br>Removed. |
| `four` | member four | ![Deprecated](https://img.shields.io/badge/-deprecated-red)<br>Removed. |
<!-- endsemconv -->

<!-- semconv stable_metric(metric_table) -->
| Name     | Instrument Type | Unit (UCUM) | Description    | Stability |
| -------- | --------------- | ----------- | -------------- | --------- |
| `stable_metric` | Histogram | `s` | stable_metric | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
<!-- endsemconv -->

<!-- semconv experimental_metric(metric_table) -->
| Name     | Instrument Type | Unit (UCUM) | Description    | Stability |
| -------- | --------------- | ----------- | -------------- | --------- |
| `experimental_metric` | Counter | `{e}` | experimental_metric | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
<!-- endsemconv -->

<!-- semconv deprecated_metric(metric_table) -->
| Name     | Instrument Type | Unit (UCUM) | Description    | Stability |
| -------- | --------------- | ----------- | -------------- | --------- |
| `deprecated_metric` | UpDownCounter | `{d}` | deprecated_metric | ![Deprecated](https://img.shields.io/badge/-deprecated-red)<br>Removed. |
<!-- endsemconv -->
