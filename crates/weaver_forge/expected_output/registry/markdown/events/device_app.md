# Events: `device.app`

This document describes the `device.app` events.

## `device.app.lifecycle`

This event represents an occurrence of a lifecycle transition on Android or iOS platform.

This event identifies the fields that are common to all lifecycle events for android and iOS using the `android.state` and `ios.state` fields. The `android.state` and `ios.state` attributes are mutually exclusive.

| Property | Value |
|----------|-------|
| Event Name | `device.app.lifecycle` |
| Stability | Development |

