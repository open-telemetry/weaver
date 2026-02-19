# Spans: `sql`

This document describes the `sql` spans.

## `db.sql`

Call-level attributes for SQL databases

| Property | Value |
|----------|-------|
| Span Kind | client |
| Stability | Development |

### Attributes

| Attribute | Type | Requirement Level | Description |
|-----------|------|-------------------|-------------|
| `db.system` | Enum | Required | An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.
 |
| `db.name` | `string` | Conditionally Required - If applicable. | This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).
 |
| `db.operation` | `string` | Conditionally Required - If `db.statement` is not applicable. | The name of the operation being executed, e.g. the [MongoDB command name](https://docs.mongodb.com/manual/reference/command/#database-operations) such as `findAndModify`, or the SQL keyword.
 |
| `server.port` | `int` | Conditionally Required - If using a port other than the default port for this DBMS and if `server.address` is set. | Server port number.
 |
| `db.connection_string` | `string` | Recommended | The connection string used to connect to the database. It is recommended to remove embedded credentials.
 |
| `db.instance.id` | `string` | Recommended - If different from the `server.address` | An identifier (address, unique name, or any other identifier) of the database instance that is executing queries or mutations on the current connection. This is useful in cases where the database is running in a clustered environment and the instrumentation is able to record the node executing the query. The client may obtain this value in databases like MySQL using queries like `select @@hostname`.
 |
| `db.jdbc.driver_classname` | `string` | Recommended | The fully-qualified class name of the [Java Database Connectivity (JDBC)](https://docs.oracle.com/javase/8/docs/technotes/guides/jdbc/) driver used to connect.
 |
| `db.sql.table` | `string` | Recommended | The name of the primary table that the operation is acting upon, including the database name (if applicable).
 |
| `db.statement` | `string` | Recommended - Should be collected by default only if there is sanitization that excludes sensitive information.
 | The database statement being executed.
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

