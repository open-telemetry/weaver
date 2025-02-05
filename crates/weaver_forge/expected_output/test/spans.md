# Semantic Convention Span Groups


## Namespace Spans `cassandra`


## Span `db.cassandra`

Call-level attributes for Cassandra

Prefix: 
Kind: client

### Attributes


#### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Tag: connection-level
  
- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]
  
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
  
- Stability: Stable
  

#### Attribute `db.cassandra.coordinator.dc`

The data center of the coordinating node for a query.



- Requirement Level: Recommended
  
- Tag: call-level-tech-specific-cassandra
  
- Type: string
- Examples: us-west-2
  
- Stability: Stable
  

#### Attribute `db.cassandra.coordinator.id`

The ID of the coordinating node for a query.



- Requirement Level: Recommended
  
- Tag: call-level-tech-specific-cassandra
  
- Type: string
- Examples: be13faa2-8574-4d71-926d-27f16cf8a7af
  
- Stability: Stable
  

#### Attribute `db.cassandra.idempotence`

Whether or not the query is idempotent.



- Requirement Level: Recommended
  
- Tag: call-level-tech-specific-cassandra
  
- Type: boolean
  
- Stability: Stable
  

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
  
- Stability: Stable
  

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
  



## Namespace Spans `cosmosdb`


## Span `db.cosmosdb`

Call-level attributes for Cosmos DB.

Prefix: 
Kind: client

### Attributes


#### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Tag: connection-level
  
- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]
  
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
  

#### Attribute `db.name`

This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).



In some SQL databases, the database name to be used is called "schema name". In case there are multiple layers that could be considered for database name (e.g. Oracle instance name and schema name), the database name to be used is the more specific layer (e.g. Oracle schema name).

- Requirement Level: Conditionally Required - If applicable.
  
- Tag: call-level
  
- Type: string
- Examples: [
    "customers",
    "main",
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
  

#### Attribute `db.cosmosdb.client_id`

Unique Cosmos client instance id.


- Requirement Level: Recommended
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: 3ba4827d-4422-483f-b59f-85b74211c11d
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.connection_mode`

Cosmos client connection mode.


- Requirement Level: Conditionally Required - if not `direct` (or pick gw as default)
  
- Tag: call-level-tech-specific
  
- Type: Enum [gateway, direct]
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.container`

Cosmos DB container name.


- Requirement Level: Conditionally Required - if available
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: anystring
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.operation_type`

CosmosDB Operation Type.


- Requirement Level: Conditionally Required - when performing one of the operations in this list
  
- Tag: call-level-tech-specific
  
- Type: Enum [Invalid, Create, Patch, Read, ReadFeed, Delete, Replace, Execute, Query, Head, HeadFeed, Upsert, Batch, QueryPlan, ExecuteJavaScript]
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.request_charge`

RU consumed for that operation


- Requirement Level: Conditionally Required - when available
  
- Tag: call-level-tech-specific
  
- Type: double
- Examples: [
    46.18,
    1.0,
]
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.request_content_length`

Request payload size in bytes


- Requirement Level: Recommended
  
- Tag: call-level-tech-specific
  
- Type: int
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.status_code`

Cosmos DB status code.


- Requirement Level: Conditionally Required - if response was received
  
- Tag: call-level-tech-specific
  
- Type: int
- Examples: [
    200,
    201,
]
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.sub_status_code`

Cosmos DB sub status code.


- Requirement Level: Conditionally Required - when response was received and contained sub-code.
  
- Tag: call-level-tech-specific
  
- Type: int
- Examples: [
    1000,
    1002,
]
  
- Stability: Stable
  

#### Attribute `user_agent.original`

Full user-agent string is generated by Cosmos DB SDK


The user-agent value is generated by SDK which is a combination of<br> `sdk_version` : Current version of SDK. e.g. 'cosmos-netstandard-sdk/3.23.0'<br> `direct_pkg_version` : Direct package version used by Cosmos DB SDK. e.g. '3.23.1'<br> `number_of_client_instances` : Number of cosmos client instances created by the application. e.g. '1'<br> `type_of_machine_architecture` : Machine architecture. e.g. 'X64'<br> `operating_system` : Operating System. e.g. 'Linux 5.4.0-1098-azure 104 18'<br> `runtime_framework` : Runtime Framework. e.g. '.NET Core 3.1.32'<br> `failover_information` : Generated key to determine if region failover enabled.
   Format Reg-{D (Disabled discovery)}-S(application region)|L(List of preferred regions)|N(None, user did not configure it).
   Default value is "NS".

- Requirement Level: Recommended
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "cosmos-netstandard-sdk/3.23.0\\|3.23.1\\|1\\|X64\\|Linux 5.4.0-1098-azure 104 18\\|.NET Core 3.1.32\\|S\\|",
]
  
- Stability: Stable
  



## Namespace Spans `couchdb`


## Span `db.couchdb`

Call-level attributes for CouchDB

Prefix: 
Kind: client

### Attributes


#### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Tag: connection-level
  
- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]
  
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
  

#### Attribute `db.name`

This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).



In some SQL databases, the database name to be used is called "schema name". In case there are multiple layers that could be considered for database name (e.g. Oracle instance name and schema name), the database name to be used is the more specific layer (e.g. Oracle schema name).

- Requirement Level: Conditionally Required - If applicable.
  
- Tag: call-level
  
- Type: string
- Examples: [
    "customers",
    "main",
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
  

#### Attribute `db.operation`

The HTTP method + the target REST route.



In **CouchDB**, `db.operation` should be set to the HTTP method + the target REST route according to the API reference documentation. For example, when retrieving a document, `db.operation` would be set to (literally, i.e., without replacing the placeholders with concrete values): [`GET /{db}/{docid}`](http://docs.couchdb.org/en/stable/api/document/common.html#get--db-docid).

- Requirement Level: Conditionally Required - If `db.statement` is not applicable.
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "GET /{db}/{docid}",
]
  
- Stability: Stable
  



## Namespace Spans `elasticsearch`


## Span `db.elasticsearch`

Call-level attributes for Elasticsearch

Prefix: 
Kind: client

### Attributes


#### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Tag: connection-level
  
- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]
  
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
  

#### Attribute `db.name`

This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).



In some SQL databases, the database name to be used is called "schema name". In case there are multiple layers that could be considered for database name (e.g. Oracle instance name and schema name), the database name to be used is the more specific layer (e.g. Oracle schema name).

- Requirement Level: Conditionally Required - If applicable.
  
- Tag: call-level
  
- Type: string
- Examples: [
    "customers",
    "main",
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
  

#### Attribute `db.elasticsearch.cluster.name`

Represents the identifier of an Elasticsearch cluster.



- Requirement Level: Optional
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "e9106fc68e3044f0b1475b04bf4ffd5f",
]
  
- Stability: Stable
  

#### Attribute `db.elasticsearch.node.name`

Represents the human-readable identifier of the node/instance to which a request was routed.



- Requirement Level: Optional
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "instance-0000000001",
]
  
- Stability: Stable
  

#### Attribute `db.elasticsearch.path_parts`

A dynamic value in the url path.



Many Elasticsearch url paths allow dynamic values. These SHOULD be recorded in span attributes in the format `db.elasticsearch.path_parts.<key>`, where `<key>` is the url path part name. The implementation SHOULD reference the [elasticsearch schema](https://raw.githubusercontent.com/elastic/elasticsearch-specification/main/output/schema/schema.json) in order to map the path part values to their names.

- Requirement Level: Conditionally Required - when the url has dynamic values
  
- Tag: call-level-tech-specific
  
- Type: template[string]
- Examples: [
    "db.elasticsearch.path_parts.index=test-index",
    "db.elasticsearch.path_parts.doc_id=123",
]
  
- Stability: Stable
  

#### Attribute `db.operation`

The endpoint identifier for the request.


When setting this to an SQL keyword, it is not recommended to attempt any client-side parsing of `db.statement` just to get this property, but it should be set if the operation name is provided by the library being instrumented. If the SQL statement has an ambiguous operation, or performs more than one operation, this value may be omitted.

- Requirement Level: Required
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "search",
    "ml.close_job",
    "cat.aliases",
]
  
- Stability: Stable
  

#### Attribute `db.statement`

The request body for a [search-type query](https://www.elastic.co/guide/en/elasticsearch/reference/current/search.html), as a json string.


- Requirement Level: Optional
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "\"{\\\"query\\\":{\\\"term\\\":{\\\"user.id\\\":\\\"kimchy\\\"}}}\"",
]
  
- Stability: Stable
  

#### Attribute `http.request.method`

HTTP request method.


HTTP request method value SHOULD be "known" to the instrumentation.
By default, this convention defines "known" methods as the ones listed in [RFC9110](https://www.rfc-editor.org/rfc/rfc9110.html#name-methods)
and the PATCH method defined in [RFC5789](https://www.rfc-editor.org/rfc/rfc5789.html).

If the HTTP request method is not known to instrumentation, it MUST set the `http.request.method` attribute to `_OTHER`.

If the HTTP instrumentation could end up converting valid HTTP request methods to `_OTHER`, then it MUST provide a way to override
the list of known HTTP methods. If this override is done via environment variable, then the environment variable MUST be named
OTEL_INSTRUMENTATION_HTTP_KNOWN_METHODS and support a comma-separated list of case-sensitive known HTTP methods
(this list MUST be a full override of the default known method, it is not a list of known methods in addition to the defaults).

HTTP method names are case-sensitive and `http.request.method` attribute value MUST match a known HTTP method name exactly.
Instrumentations for specific web frameworks that consider HTTP methods to be case insensitive, SHOULD populate a canonical equivalent.
Tracing instrumentations that do so, MUST also set `http.request.method_original` to the original value.

- Requirement Level: Required
  
- Tag: call-level-tech-specific
  
- Type: Enum [CONNECT, DELETE, GET, HEAD, OPTIONS, PATCH, POST, PUT, TRACE, _OTHER]
- Examples: [
    "GET",
    "POST",
    "HEAD",
]
  
- Stability: Stable
  

#### Attribute `server.address`

Name of the database host.



When observed from the client side, and when communicating through an intermediary, `server.address` SHOULD represent the server address behind any intermediaries, for example proxies, if it's available.

- Requirement Level: Recommended
  
- Tag: call-level-tech-specific
  
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
  
- Tag: call-level-tech-specific
  
- Type: int
- Examples: [
    80,
    8080,
    443,
]
  
- Stability: Stable
  

#### Attribute `url.full`

Absolute URL describing a network resource according to [RFC3986](https://www.rfc-editor.org/rfc/rfc3986)


For network calls, URL usually has `scheme://host[:port][path][?query][#fragment]` format, where the fragment is not transmitted over HTTP, but if it is known, it SHOULD be included nevertheless.
`url.full` MUST NOT contain credentials passed via URL in form of `https://username:password@www.example.com/`. In such case username and password SHOULD be redacted and attribute's value SHOULD be `https://REDACTED:REDACTED@www.example.com/`.
`url.full` SHOULD capture the absolute URL when it is available (or can be reconstructed) and SHOULD NOT be validated or modified except for sanitizing purposes.

- Requirement Level: Required
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "https://localhost:9200/index/_search?q=user.id:kimchy",
]
  
- Stability: Stable
  



## Namespace Spans `hbase`


## Span `db.hbase`

Call-level attributes for HBase

Prefix: 
Kind: client

### Attributes


#### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Tag: connection-level
  
- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]
  
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
  

#### Attribute `db.name`

The HBase namespace.



For HBase the `db.name` should be set to the HBase namespace.

- Requirement Level: Conditionally Required - If applicable.
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "mynamespace",
]
  
- Stability: Stable
  



## Namespace Spans `mongodb`


## Span `db.mongodb`

Call-level attributes for MongoDB

Prefix: 
Kind: client

### Attributes


#### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Tag: connection-level
  
- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]
  
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
  

#### Attribute `db.name`

This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).



In some SQL databases, the database name to be used is called "schema name". In case there are multiple layers that could be considered for database name (e.g. Oracle instance name and schema name), the database name to be used is the more specific layer (e.g. Oracle schema name).

- Requirement Level: Conditionally Required - If applicable.
  
- Tag: call-level
  
- Type: string
- Examples: [
    "customers",
    "main",
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
  

#### Attribute `db.mongodb.collection`

The MongoDB collection being accessed within the database stated in `db.name`.



- Requirement Level: Required
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "customers",
    "products",
]
  
- Stability: Stable
  



## Namespace Spans `mssql`


## Span `db.mssql`

Connection-level attributes for Microsoft SQL Server

Prefix: 
Kind: client

### Attributes


#### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Tag: connection-level
  
- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]
  
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
  

#### Attribute `db.name`

This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).



In some SQL databases, the database name to be used is called "schema name". In case there are multiple layers that could be considered for database name (e.g. Oracle instance name and schema name), the database name to be used is the more specific layer (e.g. Oracle schema name).

- Requirement Level: Conditionally Required - If applicable.
  
- Tag: call-level
  
- Type: string
- Examples: [
    "customers",
    "main",
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
  

#### Attribute `db.mssql.instance_name`

The Microsoft SQL Server [instance name](https://docs.microsoft.com/sql/connect/jdbc/building-the-connection-url?view=sql-server-ver15) connecting to. This name is used to determine the port of a named instance.



If setting a `db.mssql.instance_name`, `server.port` is no longer required (but still recommended if non-standard).

- Requirement Level: Recommended
  
- Tag: connection-level-tech-specific
  
- Type: string
- Examples: MSSQLSERVER
  
- Stability: Stable
  



## Namespace Spans `other`


## Span `db`

This document defines the attributes used to perform database client calls.

Prefix: 
Kind: client

### Attributes


#### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Tag: connection-level
  
- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]
  
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
  

#### Attribute `db.name`

This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).



In some SQL databases, the database name to be used is called "schema name". In case there are multiple layers that could be considered for database name (e.g. Oracle instance name and schema name), the database name to be used is the more specific layer (e.g. Oracle schema name).

- Requirement Level: Conditionally Required - If applicable.
  
- Tag: call-level
  
- Type: string
- Examples: [
    "customers",
    "main",
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
  



## Namespace Spans `redis`


## Span `db.redis`

Call-level attributes for Redis

Prefix: 
Kind: client

### Attributes


#### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Tag: connection-level
  
- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]
  
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
  

#### Attribute `db.name`

This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).



In some SQL databases, the database name to be used is called "schema name". In case there are multiple layers that could be considered for database name (e.g. Oracle instance name and schema name), the database name to be used is the more specific layer (e.g. Oracle schema name).

- Requirement Level: Conditionally Required - If applicable.
  
- Tag: call-level
  
- Type: string
- Examples: [
    "customers",
    "main",
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
  

#### Attribute `db.redis.database_index`

The index of the database being accessed as used in the [`SELECT` command](https://redis.io/commands/select), provided as an integer. To be used instead of the generic `db.name` attribute.



- Requirement Level: Conditionally Required - If other than the default database (`0`).
  
- Tag: call-level-tech-specific
  
- Type: int
- Examples: [
    0,
    1,
    15,
]
  
- Stability: Stable
  

#### Attribute `db.statement`

The full syntax of the Redis CLI command.



For **Redis**, the value provided for `db.statement` SHOULD correspond to the syntax of the Redis CLI. If, for example, the [`HMSET` command](https://redis.io/commands/hmset) is invoked, `"HMSET myhash field1 'Hello' field2 'World'"` would be a suitable value for `db.statement`.

- Requirement Level: Optional
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "HMSET myhash field1 'Hello' field2 'World'",
]
  
- Stability: Stable
  



## Namespace Spans `sql`


## Span `db.sql`

Call-level attributes for SQL databases

Prefix: 
Kind: client

### Attributes


#### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Tag: connection-level
  
- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]
  
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
  

#### Attribute `db.name`

This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).



In some SQL databases, the database name to be used is called "schema name". In case there are multiple layers that could be considered for database name (e.g. Oracle instance name and schema name), the database name to be used is the more specific layer (e.g. Oracle schema name).

- Requirement Level: Conditionally Required - If applicable.
  
- Tag: call-level
  
- Type: string
- Examples: [
    "customers",
    "main",
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
  

#### Attribute `db.sql.table`

The name of the primary table that the operation is acting upon, including the database name (if applicable).


It is not recommended to attempt any client-side parsing of `db.statement` just to get this property, but it should be set if it is provided by the library being instrumented. If the operation is acting upon an anonymous table, or more than one table, this value MUST NOT be set.

- Requirement Level: Recommended
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "public.users",
    "customers",
]
  
- Stability: Stable
  



## Namespace Spans `tech`


## Span `db.tech`

Semantic convention group for specific technologies

Prefix: 
Kind: client

### Attributes


#### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Tag: connection-level
  
- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]
  
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
  

#### Attribute `db.name`

This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).



In some SQL databases, the database name to be used is called "schema name". In case there are multiple layers that could be considered for database name (e.g. Oracle instance name and schema name), the database name to be used is the more specific layer (e.g. Oracle schema name).

- Requirement Level: Conditionally Required - If applicable.
  
- Tag: call-level
  
- Type: string
- Examples: [
    "customers",
    "main",
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
  
- Stability: Stable
  

#### Attribute `db.cassandra.coordinator.dc`

The data center of the coordinating node for a query.



- Requirement Level: Recommended
  
- Tag: call-level-tech-specific-cassandra
  
- Type: string
- Examples: us-west-2
  
- Stability: Stable
  

#### Attribute `db.cassandra.coordinator.id`

The ID of the coordinating node for a query.



- Requirement Level: Recommended
  
- Tag: call-level-tech-specific-cassandra
  
- Type: string
- Examples: be13faa2-8574-4d71-926d-27f16cf8a7af
  
- Stability: Stable
  

#### Attribute `db.cassandra.idempotence`

Whether or not the query is idempotent.



- Requirement Level: Recommended
  
- Tag: call-level-tech-specific-cassandra
  
- Type: boolean
  
- Stability: Stable
  

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
  
- Stability: Stable
  

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
  

#### Attribute `db.redis.database_index`

The index of the database being accessed as used in the [`SELECT` command](https://redis.io/commands/select), provided as an integer. To be used instead of the generic `db.name` attribute.



- Requirement Level: Conditionally Required - If other than the default database (`0`).
  
- Tag: call-level-tech-specific
  
- Type: int
- Examples: [
    0,
    1,
    15,
]
  
- Stability: Stable
  

#### Attribute `db.statement`

The full syntax of the Redis CLI command.



For **Redis**, the value provided for `db.statement` SHOULD correspond to the syntax of the Redis CLI. If, for example, the [`HMSET` command](https://redis.io/commands/hmset) is invoked, `"HMSET myhash field1 'Hello' field2 'World'"` would be a suitable value for `db.statement`.

- Requirement Level: Optional
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "HMSET myhash field1 'Hello' field2 'World'",
]
  
- Stability: Stable
  

#### Attribute `db.mongodb.collection`

The MongoDB collection being accessed within the database stated in `db.name`.



- Requirement Level: Required
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "customers",
    "products",
]
  
- Stability: Stable
  

#### Attribute `db.sql.table`

The name of the primary table that the operation is acting upon, including the database name (if applicable).


It is not recommended to attempt any client-side parsing of `db.statement` just to get this property, but it should be set if it is provided by the library being instrumented. If the operation is acting upon an anonymous table, or more than one table, this value MUST NOT be set.

- Requirement Level: Recommended
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "public.users",
    "customers",
]
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.client_id`

Unique Cosmos client instance id.


- Requirement Level: Recommended
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: 3ba4827d-4422-483f-b59f-85b74211c11d
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.connection_mode`

Cosmos client connection mode.


- Requirement Level: Conditionally Required - if not `direct` (or pick gw as default)
  
- Tag: call-level-tech-specific
  
- Type: Enum [gateway, direct]
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.container`

Cosmos DB container name.


- Requirement Level: Conditionally Required - if available
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: anystring
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.operation_type`

CosmosDB Operation Type.


- Requirement Level: Conditionally Required - when performing one of the operations in this list
  
- Tag: call-level-tech-specific
  
- Type: Enum [Invalid, Create, Patch, Read, ReadFeed, Delete, Replace, Execute, Query, Head, HeadFeed, Upsert, Batch, QueryPlan, ExecuteJavaScript]
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.request_charge`

RU consumed for that operation


- Requirement Level: Conditionally Required - when available
  
- Tag: call-level-tech-specific
  
- Type: double
- Examples: [
    46.18,
    1.0,
]
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.request_content_length`

Request payload size in bytes


- Requirement Level: Recommended
  
- Tag: call-level-tech-specific
  
- Type: int
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.status_code`

Cosmos DB status code.


- Requirement Level: Conditionally Required - if response was received
  
- Tag: call-level-tech-specific
  
- Type: int
- Examples: [
    200,
    201,
]
  
- Stability: Stable
  

#### Attribute `db.cosmosdb.sub_status_code`

Cosmos DB sub status code.


- Requirement Level: Conditionally Required - when response was received and contained sub-code.
  
- Tag: call-level-tech-specific
  
- Type: int
- Examples: [
    1000,
    1002,
]
  
- Stability: Stable
  

#### Attribute `user_agent.original`

Full user-agent string is generated by Cosmos DB SDK


The user-agent value is generated by SDK which is a combination of<br> `sdk_version` : Current version of SDK. e.g. 'cosmos-netstandard-sdk/3.23.0'<br> `direct_pkg_version` : Direct package version used by Cosmos DB SDK. e.g. '3.23.1'<br> `number_of_client_instances` : Number of cosmos client instances created by the application. e.g. '1'<br> `type_of_machine_architecture` : Machine architecture. e.g. 'X64'<br> `operating_system` : Operating System. e.g. 'Linux 5.4.0-1098-azure 104 18'<br> `runtime_framework` : Runtime Framework. e.g. '.NET Core 3.1.32'<br> `failover_information` : Generated key to determine if region failover enabled.
   Format Reg-{D (Disabled discovery)}-S(application region)|L(List of preferred regions)|N(None, user did not configure it).
   Default value is "NS".

- Requirement Level: Recommended
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "cosmos-netstandard-sdk/3.23.0\\|3.23.1\\|1\\|X64\\|Linux 5.4.0-1098-azure 104 18\\|.NET Core 3.1.32\\|S\\|",
]
  
- Stability: Stable
  



  