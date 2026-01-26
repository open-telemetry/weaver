# Spans: `other`

This document describes the `other` spans.

## `db`

This document defines the attributes used to perform database client calls.

| Property | Value |
|----------|-------|
| Span Kind | client |
| Stability | Development |

### Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `db.system` | Enum | **Yes** | An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers. |
| `db.connection_string` | `string` | No | The connection string used to connect to the database. It is recommended to remove embedded credentials. |
| `db.user` | `string` | No | Username for accessing the database. |
| `db.jdbc.driver_classname` | `string` | No | The fully-qualified class name of the [Java Database Connectivity (JDBC)](https://docs.oracle.com/javase/8/docs/technotes/guides/jdbc/) driver used to connect. |
| `db.name` | `string` | Conditional | This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails). |
| `db.statement` | `string` | No | The database statement being executed. |
| `db.operation` | `string` | Conditional | The name of the operation being executed, e.g. the [MongoDB command name](https://docs.mongodb.com/manual/reference/command/#database-operations) such as `findAndModify`, or the SQL keyword. |
| `server.address` | `string` | No | Name of the database host. |
| `server.port` | `int` | Conditional | Server port number. |
| `network.peer.address` | `string` | No | Peer address of the network connection - IP address or Unix domain socket name. |
| `network.peer.port` | `int` | No | Peer port number of the network connection. |
| `network.transport` | Enum | No | [OSI transport layer](https://osi-model.com/transport-layer/) or [inter-process communication method](https://wikipedia.org/wiki/Inter-process_communication). |
| `network.type` | Enum | No | [OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent. |
| `db.instance.id` | `string` | No | An identifier (address, unique name, or any other identifier) of the database instance that is executing queries or mutations on the current connection. This is useful in cases where the database is running in a clustered environment and the instrumentation is able to record the node executing the query. The client may obtain this value in databases like MySQL using queries like `select @@hostname`. |

