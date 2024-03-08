# Group `ios.lifecycle.events` (event)

## Brief

This event represents an occurrence of a lifecycle transition on the iOS platform.

Prefix: ios
Name: device.app.lifecycle

## Attributes


### Attribute `ios.state`

This attribute represents the state the application has transitioned into at the occurrence of the event.



The iOS lifecycle states are defined in the [UIApplicationDelegate documentation](https://developer.apple.com/documentation/uikit/uiapplicationdelegate#1656902), and from which the `OS terminology` column values are derived.

- Requirement Level: Required
  
- Type: Enum [active, inactive, background, foreground, terminate]
  
- Stability: Experimental
  
  