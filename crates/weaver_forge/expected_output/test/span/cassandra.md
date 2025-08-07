## Namespace Span `cassandra`


## Span `db.cassandra`

Call-level attributes for Cassandra

Prefix: 
Kind: client

### Attributes


#### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Tag: connection-level
  
- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, azure.cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]
  
- Stability: Stable
  

#### Attribute `db.connection_string`

The connection string used to connect to the database. It is recommended to remove embedded credentials.



- Requirement Level: Recommended
  
- Tag: connection-level
  
- Type: string
- Examples: Server=(localdb)\v11.0;Integrated Security=true;
  
- Stability: Stable
  

#### Attribute `db.user`

Username for accessing the database.



- Requirement Level: Recommended
  
- Tag: connection-level
  
- Type: string
- Examples: [
    "readonly_user",
    "reporting_user",
]
  
- Stability: Stable
  

#### Attribute `db.jdbc.driver_classname`

The fully-qualified class name of the [Java Database Connectivity (JDBC)](https://docs.oracle.com/javase/8/docs/technotes/guides/jdbc/) driver used to connect.



- Requirement Level: Recommended
  
- Tag: connection-level-tech-specific
  
- Type: string
- Examples: [
    "org.postgresql.Driver",
    "com.microsoft.sqlserver.jdbc.SQLServerDriver",
]
  
- Stability: Stable
  

#### Attribute `db.statement`

The database statement being executed.



- Requirement Level: Optional
  
- Tag: call-level
  
- Type: string
- Examples: [
    "SELECT * FROM wuser_table",
    "SET mykey \"WuValue\"",
]
  
- Stability: Stable
  

#### Attribute `db.operation`

The name of the operation being executed, e.g. the [MongoDB command name](https://docs.mongodb.com/manual/reference/command/#database-operations) such as `findAndModify`, or the SQL keyword.



When setting this to an SQL keyword, it is not recommended to attempt any client-side parsing of `db.statement` just to get this property, but it should be set if the operation name is provided by the library being instrumented. If the SQL statement has an ambiguous operation, or performs more than one operation, this value may be omitted.

- Requirement Level: Conditionally Required - If `db.statement` is not applicable.
  
- Tag: call-level
  
- Type: string
- Examples: [
    "findAndModify",
    "HMSET",
    "SELECT",
]
  
- Stability: Stable
  

#### Attribute `server.address`

Name of the database host.



When observed from the client side, and when communicating through an intermediary, `server.address` SHOULD represent the server address behind any intermediaries, for example proxies, if it's available.

- Requirement Level: Recommended
  
- Tag: connection-level
  
- Type: string
- Examples: [
    "example.com",
    "10.1.2.80",
    "/tmp/my.sock",
]
  
- Stability: Stable
  

#### Attribute `server.port`

Server port number.


When observed from the client side, and when communicating through an intermediary, `server.port` SHOULD represent the server port behind any intermediaries, for example proxies, if it's available.

- Requirement Level: Conditionally Required - If using a port other than the default port for this DBMS and if `server.address` is set.
  
- Tag: connection-level
  
- Type: int
- Examples: [
    80,
    8080,
    443,
]
  
- Stability: Stable
  

#### Attribute `network.peer.address`

Peer address of the network connection - IP address or Unix domain socket name.


- Requirement Level: Recommended
  
- Tag: connection-level
  
- Type: string
- Examples: [
    "10.1.2.80",
    "/tmp/my.sock",
]
  
- Stability: Stable
  

#### Attribute `network.peer.port`

Peer port number of the network connection.


- Requirement Level: Optional
  
- Tag: connection-level
  
- Type: int
- Examples: [
    65123,
]
  
- Stability: Stable
  

#### Attribute `network.transport`

[OSI transport layer](https://osi-model.com/transport-layer/) or [inter-process communication method](https://wikipedia.org/wiki/Inter-process_communication).



The value SHOULD be normalized to lowercase.

Consider always setting the transport when setting a port number, since
a port number is ambiguous without knowing the transport. For example
different processes could be listening on TCP port 12345 and UDP port 12345.

- Requirement Level: Recommended
  
- Tag: connection-level
  
- Type: Enum [tcp, udp, pipe, unix]
- Examples: [
    "tcp",
    "udp",
]
  
- Stability: Stable
  

#### Attribute `network.type`

[OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent.


The value SHOULD be normalized to lowercase.

- Requirement Level: Recommended
  
- Tag: connection-level
  
- Type: Enum [ipv4, ipv6]
- Examples: [
    "ipv4",
    "ipv6",
]
  
- Stability: Stable
  

#### Attribute `db.instance.id`

An identifier (address, unique name, or any other identifier) of the database instance that is executing queries or mutations on the current connection. This is useful in cases where the database is running in a clustered environment and the instrumentation is able to record the node executing the query. The client may obtain this value in databases like MySQL using queries like `select @@hostname`.



- Requirement Level: Optional
  
- Tag: connection-level
  
- Type: string
- Examples: mysql-e26b99z.example.com
  
- Stability: Stable
  

#### Attribute `db.cassandra.consistency_level`

The consistency level of the query. Based on consistency values from [CQL](https://docs.datastax.com/en/cassandra-oss/3.0/cassandra/dml/dmlConfigConsistency.html).



- Requirement Level: Recommended
  
- Tag: call-level-tech-specific-cassandra
  
- Type: Enum [all, each_quorum, quorum, local_quorum, one, two, three, local_one, any, serial, local_serial]
  
- Stability: Development
  

#### Attribute `db.cassandra.coordinator.dc`

The data center of the coordinating node for a query.



- Requirement Level: Recommended
  
- Tag: call-level-tech-specific-cassandra
  
- Type: string
- Examples: us-west-2
  
- Stability: Release_candidate
  

#### Attribute `db.cassandra.coordinator.id`

The ID of the coordinating node for a query.



- Requirement Level: Recommended
  
- Tag: call-level-tech-specific-cassandra
  
- Type: string
- Examples: be13faa2-8574-4d71-926d-27f16cf8a7af
  
- Stability: Development
  

#### Attribute `db.cassandra.idempotence`

Whether or not the query is idempotent.



- Requirement Level: Recommended
  
- Tag: call-level-tech-specific-cassandra
  
- Type: boolean
  
- Stability: Alpha
  

#### Attribute `db.cassandra.page_size`

The fetch size used for paging, i.e. how many rows will be returned at once.



- Requirement Level: Recommended
  
- Tag: call-level-tech-specific-cassandra
  
- Type: int
- Examples: [
    5000,
]
  
- Stability: Stable
  

#### Attribute `db.cassandra.speculative_execution_count`

The number of times a query was speculatively executed. Not set or `0` if the query was not executed speculatively.



- Requirement Level: Recommended
  
- Tag: call-level-tech-specific-cassandra
  
- Type: int
- Examples: [
    0,
    2,
]
  
- Stability: Beta
  

#### Attribute `db.cassandra.table`

The name of the primary Cassandra table that the operation is acting upon, including the keyspace name (if applicable).


This mirrors the db.sql.table attribute but references cassandra rather than sql. It is not recommended to attempt any client-side parsing of `db.statement` just to get this property, but it should be set if it is provided by the library being instrumented. If the operation is acting upon an anonymous table, or more than one table, this value MUST NOT be set.

- Requirement Level: Recommended
  
- Tag: call-level-tech-specific-cassandra
  
- Type: string
- Examples: mytable
  
- Stability: Stable
  

#### Attribute `db.name`

The keyspace name in Cassandra.



For Cassandra the `db.name` should be set to the Cassandra keyspace name.

- Requirement Level: Conditionally Required - If applicable.
  
- Tag: call-level-tech-specific-cassandra
  
- Type: string
- Examples: [
    "mykeyspace",
]
  
- Stability: Stable
  

 