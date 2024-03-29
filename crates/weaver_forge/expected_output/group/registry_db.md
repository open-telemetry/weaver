# Group `registry.db` (attribute_group)

## Brief

This document defines the attributes used to describe telemetry in the context of databases.

prefix: db

## Attributes


### Attribute `db.cassandra.coordinator.dc`

The data center of the coordinating node for a query.



- Requirement Level: Recommended

- Tag: tech-specific-cassandra

- Type: string
- Examples: us-west-2


### Attribute `db.cassandra.coordinator.id`

The ID of the coordinating node for a query.



- Requirement Level: Recommended

- Tag: tech-specific-cassandra

- Type: string
- Examples: be13faa2-8574-4d71-926d-27f16cf8a7af


### Attribute `db.cassandra.consistency_level`

The consistency level of the query. Based on consistency values from [CQL](https://docs.datastax.com/en/cassandra-oss/3.0/cassandra/dml/dmlConfigConsistency.html).



- Requirement Level: Recommended

- Tag: tech-specific-cassandra

- Type: Enum [all, each_quorum, quorum, local_quorum, one, two, three, local_one, any, serial, local_serial]


### Attribute `db.cassandra.idempotence`

Whether or not the query is idempotent.



- Requirement Level: Recommended

- Tag: tech-specific-cassandra

- Type: boolean


### Attribute `db.cassandra.page_size`

The fetch size used for paging, i.e. how many rows will be returned at once.



- Requirement Level: Recommended

- Tag: tech-specific-cassandra

- Type: int
- Examples: [
    5000,
]


### Attribute `db.cassandra.speculative_execution_count`

The number of times a query was speculatively executed. Not set or `0` if the query was not executed speculatively.



- Requirement Level: Recommended

- Tag: tech-specific-cassandra

- Type: int
- Examples: [
    0,
    2,
]


### Attribute `db.cassandra.table`

The name of the primary Cassandra table that the operation is acting upon, including the keyspace name (if applicable).


This mirrors the db.sql.table attribute but references cassandra rather than sql. It is not recommended to attempt any client-side parsing of `db.statement` just to get this property, but it should be set if it is provided by the library being instrumented. If the operation is acting upon an anonymous table, or more than one table, this value MUST NOT be set.

- Requirement Level: Recommended

- Tag: tech-specific-cassandra

- Type: string
- Examples: mytable


### Attribute `db.connection_string`

The connection string used to connect to the database. It is recommended to remove embedded credentials.



- Requirement Level: Recommended

- Tag: db-generic

- Type: string
- Examples: Server=(localdb)\v11.0;Integrated Security=true;


### Attribute `db.cosmosdb.client_id`

Unique Cosmos client instance id.


- Requirement Level: Recommended

- Tag: tech-specific-cosmosdb

- Type: string
- Examples: 3ba4827d-4422-483f-b59f-85b74211c11d


### Attribute `db.cosmosdb.connection_mode`

Cosmos client connection mode.


- Requirement Level: Recommended

- Tag: tech-specific-cosmosdb

- Type: Enum [gateway, direct]


### Attribute `db.cosmosdb.container`

Cosmos DB container name.


- Requirement Level: Recommended

- Tag: tech-specific-cosmosdb

- Type: string
- Examples: anystring


### Attribute `db.cosmosdb.operation_type`

CosmosDB Operation Type.


- Requirement Level: Recommended

- Tag: tech-specific-cosmosdb

- Type: Enum [Invalid, Create, Patch, Read, ReadFeed, Delete, Replace, Execute, Query, Head, HeadFeed, Upsert, Batch, QueryPlan, ExecuteJavaScript]


### Attribute `db.cosmosdb.request_charge`

RU consumed for that operation


- Requirement Level: Recommended

- Tag: tech-specific-cosmosdb

- Type: double
- Examples: [
    46.18,
    1.0,
]


### Attribute `db.cosmosdb.request_content_length`

Request payload size in bytes


- Requirement Level: Recommended

- Tag: tech-specific-cosmosdb

- Type: int


### Attribute `db.cosmosdb.status_code`

Cosmos DB status code.


- Requirement Level: Recommended

- Tag: tech-specific-cosmosdb

- Type: int
- Examples: [
    200,
    201,
]


### Attribute `db.cosmosdb.sub_status_code`

Cosmos DB sub status code.


- Requirement Level: Recommended

- Tag: tech-specific-cosmosdb

- Type: int
- Examples: [
    1000,
    1002,
]


### Attribute `db.elasticsearch.cluster.name`

Represents the identifier of an Elasticsearch cluster.



- Requirement Level: Recommended

- Tag: tech-specific-elasticsearch

- Type: string
- Examples: [
    "e9106fc68e3044f0b1475b04bf4ffd5f",
]


### Attribute `db.elasticsearch.node.name`

Represents the human-readable identifier of the node/instance to which a request was routed.



- Requirement Level: Recommended

- Tag: tech-specific-elasticsearch

- Type: string
- Examples: [
    "instance-0000000001",
]


### Attribute `db.elasticsearch.path_parts`

A dynamic value in the url path.



Many Elasticsearch url paths allow dynamic values. These SHOULD be recorded in span attributes in the format `db.elasticsearch.path_parts.<key>`, where `<key>` is the url path part name. The implementation SHOULD reference the [elasticsearch schema](https://raw.githubusercontent.com/elastic/elasticsearch-specification/main/output/schema/schema.json) in order to map the path part values to their names.

- Requirement Level: Recommended

- Tag: tech-specific-elasticsearch

- Type: template[string]
- Examples: [
    "db.elasticsearch.path_parts.index=test-index",
    "db.elasticsearch.path_parts.doc_id=123",
]


### Attribute `db.jdbc.driver_classname`

The fully-qualified class name of the [Java Database Connectivity (JDBC)](https://docs.oracle.com/javase/8/docs/technotes/guides/jdbc/) driver used to connect.



- Requirement Level: Recommended

- Tag: tech-specific-jdbc

- Type: string
- Examples: [
    "org.postgresql.Driver",
    "com.microsoft.sqlserver.jdbc.SQLServerDriver",
]


### Attribute `db.mongodb.collection`

The MongoDB collection being accessed within the database stated in `db.name`.



- Requirement Level: Recommended

- Tag: tech-specific-mongodb

- Type: string
- Examples: [
    "customers",
    "products",
]


### Attribute `db.mssql.instance_name`

The Microsoft SQL Server [instance name](https://docs.microsoft.com/sql/connect/jdbc/building-the-connection-url?view=sql-server-ver15) connecting to. This name is used to determine the port of a named instance.



If setting a `db.mssql.instance_name`, `server.port` is no longer required (but still recommended if non-standard).

- Requirement Level: Recommended

- Tag: tech-specific-mssql

- Type: string
- Examples: MSSQLSERVER


### Attribute `db.name`

This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).



In some SQL databases, the database name to be used is called "schema name". In case there are multiple layers that could be considered for database name (e.g. Oracle instance name and schema name), the database name to be used is the more specific layer (e.g. Oracle schema name).

- Requirement Level: Recommended

- Tag: db-generic

- Type: string
- Examples: [
    "customers",
    "main",
]


### Attribute `db.operation`

The name of the operation being executed, e.g. the [MongoDB command name](https://docs.mongodb.com/manual/reference/command/#database-operations) such as `findAndModify`, or the SQL keyword.



When setting this to an SQL keyword, it is not recommended to attempt any client-side parsing of `db.statement` just to get this property, but it should be set if the operation name is provided by the library being instrumented. If the SQL statement has an ambiguous operation, or performs more than one operation, this value may be omitted.

- Requirement Level: Recommended

- Tag: db-generic

- Type: string
- Examples: [
    "findAndModify",
    "HMSET",
    "SELECT",
]


### Attribute `db.redis.database_index`

The index of the database being accessed as used in the [`SELECT` command](https://redis.io/commands/select), provided as an integer. To be used instead of the generic `db.name` attribute.



- Requirement Level: Recommended

- Tag: tech-specific-redis

- Type: int
- Examples: [
    0,
    1,
    15,
]


### Attribute `db.sql.table`

The name of the primary table that the operation is acting upon, including the database name (if applicable).


It is not recommended to attempt any client-side parsing of `db.statement` just to get this property, but it should be set if it is provided by the library being instrumented. If the operation is acting upon an anonymous table, or more than one table, this value MUST NOT be set.

- Requirement Level: Recommended

- Tag: tech-specific-sql

- Type: string
- Examples: [
    "public.users",
    "customers",
]


### Attribute `db.statement`

The database statement being executed.



- Requirement Level: Recommended

- Tag: db-generic

- Type: string
- Examples: [
    "SELECT * FROM wuser_table",
    "SET mykey \"WuValue\"",
]


### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Recommended

- Tag: db-generic

- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]


### Attribute `db.user`

Username for accessing the database.



- Requirement Level: Recommended

- Tag: db-generic

- Type: string
- Examples: [
    "readonly_user",
    "reporting_user",
]


### Attribute `db.instance.id`

An identifier (address, unique name, or any other identifier) of the database instance that is executing queries or mutations on the current connection. This is useful in cases where the database is running in a clustered environment and the instrumentation is able to record the node executing the query. The client may obtain this value in databases like MySQL using queries like `select @@hostname`.



- Requirement Level: Recommended

- Tag: db-generic

- Type: string
- Examples: mysql-e26b99z.example.com



## Provenance

Source file: data/registry-db.yaml

