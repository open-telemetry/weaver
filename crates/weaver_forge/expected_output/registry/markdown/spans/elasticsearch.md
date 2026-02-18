# Spans: `elasticsearch`

This document describes the `elasticsearch` spans.

## `db.elasticsearch`

Call-level attributes for Elasticsearch

| Property | Value |
|----------|-------|
| Span Kind | client |
| Stability | Development |

### Attributes

| Attribute | Type | Requirement Level | Description |
|-----------|------|-------------------|-------------|
| `db.operation` | `string` | Required | The endpoint identifier for the request.
 |
| `db.system` | Enum | Required | An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.
 |
| `http.request.method` | Enum | Required | HTTP request method.
 |
| `url.full` | `string` | Required | Absolute URL describing a network resource according to [RFC3986](https://www.rfc-editor.org/rfc/rfc3986)
 |
| `db.elasticsearch.path_parts` | `template[string]` | Conditionally Required - when the url has dynamic values | A dynamic value in the url path.
 |
| `db.name` | `string` | Conditionally Required - If applicable. | This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).
 |
| `server.port` | `int` | Conditionally Required - If using a port other than the default port for this DBMS and if `server.address` is set. | Server port number.
 |
| `db.connection_string` | `string` | Recommended | The connection string used to connect to the database. It is recommended to remove embedded credentials.
 |
| `db.elasticsearch.cluster.name` | `string` | Recommended - When communicating with an Elastic Cloud deployment, this should be collected from the "X-Found-Handling-Cluster" HTTP response header.
 | Represents the identifier of an Elasticsearch cluster.
 |
| `db.elasticsearch.node.name` | `string` | Recommended - When communicating with an Elastic Cloud deployment, this should be collected from the "X-Found-Handling-Instance" HTTP response header.
 | Represents the human-readable identifier of the node/instance to which a request was routed.
 |
| `db.instance.id` | `string` | Recommended - If different from the `server.address` | An identifier (address, unique name, or any other identifier) of the database instance that is executing queries or mutations on the current connection. This is useful in cases where the database is running in a clustered environment and the instrumentation is able to record the node executing the query. The client may obtain this value in databases like MySQL using queries like `select @@hostname`.
 |
| `db.jdbc.driver_classname` | `string` | Recommended | The fully-qualified class name of the [Java Database Connectivity (JDBC)](https://docs.oracle.com/javase/8/docs/technotes/guides/jdbc/) driver used to connect.
 |
| `db.statement` | `string` | Recommended - Should be collected by default for search-type queries and only if there is sanitization that excludes sensitive information.
 | The request body for a [search-type query](https://www.elastic.co/guide/en/elasticsearch/reference/current/search.html), as a json string.
 |
| `db.user` | `string` | Recommended | Username for accessing the database.
 |
| `network.peer.address` | `string` | Recommended | Peer address of the network connection - IP address or Unix domain socket name.
 |
| `network.peer.port` | `int` | Recommended - If `network.peer.address` is set. | Peer port number of the network connection.
 |
| `network.transport` | Enum | Recommended | [OSI transport layer](https://osi-model.com/transport-layer/) or [inter-process communication method](https://wikipedia.org/wiki/Inter-process_communication).
 |
| `network.type` | Enum | Recommended | [OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent.
 |
| `server.address` | `string` | Recommended | Name of the database host.
 |

