# Bundles

This document is a design proposal to add the ability to define bundles of attributes in semconv.

## Use cases

There are a few different use cases which bundles will help us to achieve.

### Defining signals

By introducing bundles we are able to bundle a collection of attributes together into a single reusable item. 
These items have the primary goal of reducing the need to copy paste definitions while also reducing the maintaince effort.

An example of where this would be useful is if a `server` bundle was defined similiar to below:

```
bundle:
  - name: server
    brief: Defines a server
    note:
    attributes
      - ref: sever.address
      - ref: sever.port
```

Once it is defined, it can then be used on other signals to extend the definition.

```
span:
  - name: messaging.produced
    brief: Defines a span for sending a message via a message brooker such as rabbitmq
    note:
    attributes
      - ref: message.system.name
    bundles:
      - ref: server
```

In terms of the resolved schema, these bundles are blended seamlessly in to the definition.

```
span:
  - name: messaging.produced
    brief: Defines a span for sending a message via a message brooker such as rabbitmq
    note:
    attributes
      - id: message.system.name
        ....
      - id: sever.address
        ....
      - id: sever.port
        ....
```

By adopting this approach we are improving the mantainability of the definitions,
by reusing the definition rather than copy and pasting strucutures.

### Extending signals

This use case is similiar to the previous except it is focussed on offering optional contextual extensions.
An example of this extension would be Cloud Events which are only applicable if the application is using Cloud Events, 
hence the usage is contextual based upon cloud events being used.

To indicate that a bundle is contextual, this can be done by defining this on the bundle, just like below for `cloud_events`:

```
bundle:
  - name: cloud_events
    brief: For more information on the concepts, terminology and background of CloudEvents consult the CloudEvents Primer document.
    context: Can be used when a cloud event payload is being handled (sent/recieved).
    note:
    attributes
      - ref: cloudevents.event_id
      - ref: cloudevents.event_source
      ......
```
Once it is defined, it can then be used on other signals ie the span defined in the previous chapter.

```
span:
  - name: messaging.produced
    brief: Defines a span for sending a message via a message brooker such as rabbitmq
    note:
    attributes
      - ref: message.system.name
    bundles:
      - ref: server
      - ref: cloud_events
```

In terms of the resolved schema, these bundles are blended seamlessly in to the definition.

```
span:
  - name: messaging.produced
    brief: Defines a span for sending a message via a message brooker such as rabbitmq
    note:
    attributes
      - id: message.system.name
        ....
      - id: sever.address
        ....
      - id: sever.port
        ....
      - id: cloudevents.event_id
        bundle: cloud_events
        context: Can be used when a cloud event 
        ....
      - id: cloudevents.event_source
        bundle: cloud_events
        context: Can be used when a cloud event 
        ....
```

By adopting this approach the definition of cloud events can easily be added/extended without needing to touch the usage.

## Comparison to v1 solution
Attribute bundles are similiar to attribute groups however,
there are some important differences that needs to be called out:

* **Extends:** Bundles can not extend other bundles which is the case to reduce complexity
and enable streamlining of the resolving process.
* **Referencing:** A signal ie span can now reference multiple bundles,
which removes the need for creating nested bundles.
