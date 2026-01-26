# Spans: `cassandra`

This document describes the `cassandra` spans.

## `db.cassandra`

Call-level attributes for Cassandra

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
| `db.statement` | `string` | No | The database statement being executed. |
| `db.operation` | `string` | Conditional | The name of the operation being executed, e.g. the [MongoDB command name](https://docs.mongodb.com/manual/reference/command/#database-operations) such as `findAndModify`, or the SQL keyword. |
| `server.address` | `string` | No | Name of the database host. |
| `server.port` | `int` | Conditional | Server port number. |
| `network.peer.address` | `string` | No | Peer address of the network connection - IP address or Unix domain socket name. |
| `network.peer.port` | `int` | No | Peer port number of the network connection. |
| `network.transport` | Enum | No | [OSI transport layer](https://osi-model.com/transport-layer/) or [inter-process communication method](https://wikipedia.org/wiki/Inter-process_communication). |
| `network.type` | Enum | No | [OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent. |
| `db.instance.id` | `string` | No | An identifier (address, unique name, or any other identifier) of the database instance that is executing queries or mutations on the current connection. This is useful in cases where the database is running in a clustered environment and the instrumentation is able to record the node executing the query. The client may obtain this value in databases like MySQL using queries like `select @@hostname`. |
| `db.cassandra.consistency_level` | Enum | No | The consistency level of the query. Based on consistency values from [CQL](https://docs.datastax.com/en/cassandra-oss/3.0/cassandra/dml/dmlConfigConsistency.html). |
| `db.cassandra.coordinator.dc` | `string` | No | The data center of the coordinating node for a query. |
| `db.cassandra.coordinator.id` | `string` | No | The ID of the coordinating node for a query. |
| `db.cassandra.idempotence` | `boolean` | No | Whether or not the query is idempotent. |
| `db.cassandra.page_size` | `int` | No | The fetch size used for paging, i.e. how many rows will be returned at once. |
| `db.cassandra.speculative_execution_count` | `int` | No | The number of times a query was speculatively executed. Not set or `0` if the query was not executed speculatively. |
| `db.cassandra.table` | `string` | No | The name of the primary Cassandra table that the operation is acting upon, including the keyspace name (if applicable). |
| `db.name` | `string` | Conditional | The keyspace name in Cassandra. |

