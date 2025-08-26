# Packages

This document is a design proposal to add the ability to define packages of signals in semconv.

## Use cases

One of the common questions which have come up is how we can defined requirement levels
of metrics, events etc and the answer to this is packages can provide this functionality.

### Defining Packages

The objective of packages is to define a collection of signals which is to be implemented.
Based on this a package can be defined following the below example for `http.server`:

```
package:
  - name: http.server
    brief: Defines a http server package
    attributes:
      - ref: server.address
        brief: The ip address or dns name which the http request is sent to.
    spans:
      - ref: http.server
        requirement_level: required
    metrics:
      - ref: http.server.request.duration
        requirement_level: required
      - ref: http.server.request.body.size
        requirement_level: recommended
      - ref: http.server.response.body.size
        requirement_level: recommended
```

For the registry, these packages would follow the other signals where in which the references are resolved
and also the signals would benefit from containing a list of all the usages of a signal.

This would look like the below:

```
span:
  - name: http.server
    brief:
    note:
    attributes
      - ref: http.route
    packages:
      - ref: http.server
```

## Functionality

### Overriding attribute definitions

When an attribute is specified on a package, the definition from the package will
override the definition which comes from the signal definition.

The will eliminate the need to be overriding the definition of each and 
every signal with updated notes/briefs etc but just doing it once.

This is useful for metrics which can not be extended while mantaining the id and 
hence it creates reusable metrics.

### Specifying requirment levels

By creating a group of signals, it now becomes possible to specify the 
requirment to implement the different signals alongside that reference.

## Not supported

### Extending signals
Adding attributes to signals is not a supported use case as the attributes should already be defined on the signal.
This definition could be an explicit reference or an implicit via a bundle but it should be there.

## Future

### Promoting Bundles

With the ability to define the packages we could specify bundles which should be promoted from contextual to a standard bundle.

An example of this would be if we had a db span which had a contextual bundle for vector databases.
A package for vector databases can be defined which promotes the vector database bundle to be a standard bundle.

This in action would look like the following:

```
package:
  - name: database.client
    brief: Defines a http server package
    bundles:
      - database.vector
    attributes:
      - ref: server.address
        brief: The ip address or dns name which the db request is sent to.
    spans:
      - ref: db.client
        requirement_level: required
```

and then once resolution is completed the vector attributes 
would appear just like the base attributes without any context.

This way we can have multiple packages using common definitions and extended via bundles,
without needing to be redefining the definition.

### Implementations
In the future these packages would form the basis of describing implementations and useful for implementation documentation etc.
