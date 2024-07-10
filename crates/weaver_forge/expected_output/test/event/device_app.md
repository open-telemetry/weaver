## Events Namespace `device.app`


## Event `device.app.lifecycle`

Note: This event identifies the fields that are common to all lifecycle events for android and iOS using the `android.state` and `ios.state` fields. The `android.state` and `ios.state` attributes are mutually exclusive.

Brief: This event represents an occurrence of a lifecycle transition on Android or iOS platform.

Requirement level: 
Stability: experimental

### Body Fields

#### Field `ios.state`

This attribute represents the state the application has transitioned into at the occurrence of the event.

The iOS lifecycle states are defined in the [UIApplicationDelegate documentation](https://developer.apple.com/documentation/uikit/uiapplicationdelegate#1656902), and from which the `OS terminology` column values are derived.

- Requirement Level: Conditionally Required - if and only if `os.name` is `ios`
- Type: Enum [active, inactive, background, foreground, terminate]
- Stability: Experimental

#### Field `android.state`

This attribute represents the state the application has transitioned into at the occurrence of the event.

The Android lifecycle states are defined in [Activity lifecycle callbacks](https://developer.android.com/guide/components/activities/activity-lifecycle#lc), and from which the `OS identifiers` are derived.

- Requirement Level: Conditionally Required - if and only if `os.name` is `android`
- Type: Enum [created, background, foreground]
- Stability: Experimental

### Attributes


  