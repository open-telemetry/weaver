# Group `db.mongodb` (span)

## Brief

Call-level attributes for MongoDB

prefix: 

## Attributes


### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Required

- Tag: connection-level

- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]


### Attribute `db.connection_string`

The connection string used to connect to the database. It is recommended to remove embedded credentials.



- Requirement Level: Recommended

- Tag: connection-level

- Type: string
- Examples: Server=(localdb)\v11.0;Integrated Security=true;


### Attribute `db.user`

Username for accessing the database.



- Requirement Level: Recommended

- Tag: connection-level

- Type: string
- Examples: [
    "readonly_user",
    "reporting_user",
]


### Attribute `db.jdbc.driver_classname`

The fully-qualified class name of the [Java Database Connectivity (JDBC)](https://docs.oracle.com/javase/8/docs/technotes/guides/jdbc/) driver used to connect.



- Requirement Level: Recommended

- Tag: connection-level-tech-specific

- Type: string
- Examples: [
    "org.postgresql.Driver",
    "com.microsoft.sqlserver.jdbc.SQLServerDriver",
]


### Attribute `db.name`

This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).



In some SQL databases, the database name to be used is called "schema name". In case there are multiple layers that could be considered for database name (e.g. Oracle instance name and schema name), the database name to be used is the more specific layer (e.g. Oracle schema name).

- Requirement Level: Conditionally Required - If applicable.

- Tag: call-level

- Type: string
- Examples: [
    "customers",
    "main",
]


### Attribute `db.statement`

The database statement being executed.



- Requirement Level: Optional

- Tag: call-level

- Type: string
- Examples: [
    "SELECT * FROM wuser_table",
    "SET mykey \"WuValue\"",
]


### Attribute `db.operation`

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


### Attribute `server.address`

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


### Attribute `server.port`

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


### Attribute `network.peer.address`

Peer address of the network connection - IP address or Unix domain socket name.


- Requirement Level: Recommended

- Tag: connection-level

- Type: string
- Examples: [
    "10.1.2.80",
    "/tmp/my.sock",
]

- Stability: Stable


### Attribute `network.peer.port`

Peer port number of the network connection.


- Requirement Level: Optional

- Tag: connection-level

- Type: int
- Examples: [
    65123,
]

- Stability: Stable


### Attribute `network.transport`

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


### Attribute `network.type`

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


### Attribute `db.instance.id`

An identifier (address, unique name, or any other identifier) of the database instance that is executing queries or mutations on the current connection. This is useful in cases where the database is running in a clustered environment and the instrumentation is able to record the node executing the query. The client may obtain this value in databases like MySQL using queries like `select @@hostname`.



- Requirement Level: Optional

- Tag: connection-level

- Type: string
- Examples: mysql-e26b99z.example.com


### Attribute `db.mongodb.collection`

The MongoDB collection being accessed within the database stated in `db.name`.



- Requirement Level: Required

- Tag: call-level-tech-specific

- Type: string
- Examples: [
    "customers",
    "products",
]



## Provenance

Source file: data/trace-database.yaml

attribute: db.connection_string
  - source group: db
  - inherited fields: tag
  - locally overridden fields: 

attribute: db.instance.id
  - source group: db
  - inherited fields: requirement_level, tag
  - locally overridden fields: 

attribute: db.jdbc.driver_classname
  - source group: db
  - inherited fields: tag
  - locally overridden fields: 

attribute: db.name
  - source group: db
  - inherited fields: requirement_level, tag
  - locally overridden fields: 

attribute: db.operation
  - source group: db
  - inherited fields: requirement_level, tag
  - locally overridden fields: 

attribute: db.statement
  - source group: db
  - inherited fields: requirement_level, tag
  - locally overridden fields: 

attribute: db.system
  - source group: db
  - inherited fields: requirement_level, tag
  - locally overridden fields: 

attribute: db.user
  - source group: db
  - inherited fields: tag
  - locally overridden fields: 

attribute: network.peer.address
  - source group: db
  - inherited fields: tag
  - locally overridden fields: 

attribute: network.peer.port
  - source group: db
  - inherited fields: requirement_level, tag
  - locally overridden fields: 

attribute: network.transport
  - source group: db
  - inherited fields: tag
  - locally overridden fields: 

attribute: network.type
  - source group: db
  - inherited fields: tag
  - locally overridden fields: 

attribute: server.address
  - source group: db
  - inherited fields: brief, tag
  - locally overridden fields: 

attribute: server.port
  - source group: db
  - inherited fields: requirement_level, tag
  - locally overridden fields: 

attribute: db.connection_string
  - source group: registry.db
  - inherited fields: brief, examples, note, requirement_level
  - locally overridden fields: tag

attribute: db.instance.id
  - source group: registry.db
  - inherited fields: brief, examples, note
  - locally overridden fields: requirement_level, tag

attribute: db.jdbc.driver_classname
  - source group: registry.db
  - inherited fields: brief, examples, note, requirement_level
  - locally overridden fields: tag

attribute: db.mongodb.collection
  - source group: registry.db
  - inherited fields: brief, examples, note
  - locally overridden fields: requirement_level, tag

attribute: db.name
  - source group: registry.db
  - inherited fields: brief, examples, note
  - locally overridden fields: requirement_level, tag

attribute: db.operation
  - source group: registry.db
  - inherited fields: brief, examples, note
  - locally overridden fields: requirement_level, tag

attribute: db.statement
  - source group: registry.db
  - inherited fields: brief, examples, note
  - locally overridden fields: requirement_level, tag

attribute: db.system
  - source group: registry.db
  - inherited fields: brief, note
  - locally overridden fields: requirement_level, tag

attribute: db.user
  - source group: registry.db
  - inherited fields: brief, examples, note, requirement_level
  - locally overridden fields: tag

attribute: network.peer.address
  - source group: registry.network
  - inherited fields: brief, examples, note, requirement_level, stability
  - locally overridden fields: tag

attribute: network.peer.port
  - source group: registry.network
  - inherited fields: brief, examples, note, stability
  - locally overridden fields: requirement_level, tag

attribute: network.transport
  - source group: registry.network
  - inherited fields: brief, examples, note, requirement_level, stability
  - locally overridden fields: tag

attribute: network.type
  - source group: registry.network
  - inherited fields: brief, examples, note, requirement_level, stability
  - locally overridden fields: tag

attribute: server.address
  - source group: server
  - inherited fields: examples, note, requirement_level, stability
  - locally overridden fields: brief, tag

attribute: server.port
  - source group: server
  - inherited fields: brief, examples, note, stability
  - locally overridden fields: requirement_level, tag

