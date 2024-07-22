## Events Namespace `lifecycle`


## Event `device.app.lifecycle`

Note: 
Brief: This event represents an occurrence of a lifecycle transition on the iOS platform.

Requirement level: 
Stability: 

### Attributes


#### Attribute `ios.state`

This attribute represents the state the application has transitioned into at the occurrence of the event.



The iOS lifecycle states are defined in the [UIApplicationDelegate documentation](https://developer.apple.com/documentation/uikit/uiapplicationdelegate#1656902), and from which the `OS terminology` column values are derived.

- Requirement Level: Required
  
- Type: Enum [active, inactive, background, foreground, terminate]
  
- Stability: Experimental
  
  
  
## Event `device.app.lifecycle`

Note: 
Brief: This event represents an occurrence of a lifecycle transition on the Android platform.

Requirement level: 
Stability: 

### Attributes


#### Attribute `android.state`

This attribute represents the state the application has transitioned into at the occurrence of the event.



The Android lifecycle states are defined in [Activity lifecycle callbacks](https://developer.android.com/guide/components/activities/activity-lifecycle#lc), and from which the `OS identifiers` are derived.

- Requirement Level: Required
  
- Type: Enum [created, background, foreground]
  
- Stability: Experimental
  
  
  