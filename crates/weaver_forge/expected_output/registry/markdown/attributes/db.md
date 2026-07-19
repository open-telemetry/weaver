# Attributes: `db`

This document describes the `db` attributes.

## `db.cassandra.consistency_level`

The consistency level of the query. Based on consistency values from [CQL](https://docs.datastax.com/en/cassandra-oss/3.0/cassandra/dml/dmlConfigConsistency.html).

| Property | Value |
|----------|-------|
| Type | Enum ([see values below](#enum-values)) |
| Requirement Level | Recommended |
| Stability | Development |

### Enum Values

| Value | Description | Stability |
|-------|-------------|-----------|
| `all` | - | Stable |
| `each_quorum` | - | Stable |
| `quorum` | - | Stable |
| `local_quorum` | - | Stable |
| `one` | - | Stable |
| `two` | - | Stable |
| `three` | - | Stable |
| `local_one` | - | Stable |
| `any` | - | Stable |
| `serial` | - | Stable |
| `local_serial` | - | Stable |

## `db.cassandra.coordinator.dc`

The data center of the coordinating node for a query.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Release_candidate |
| Examples | `us-west-2` |

## `db.cassandra.coordinator.id`

The ID of the coordinating node for a query.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Development |
| Examples | `be13faa2-8574-4d71-926d-27f16cf8a7af` |

## `db.cassandra.idempotence`

Whether or not the query is idempotent.

| Property | Value |
|----------|-------|
| Type | `boolean` |
| Requirement Level | Recommended |
| Stability | Alpha |

## `db.cassandra.page_size`

The fetch size used for paging, i.e. how many rows will be returned at once.

| Property | Value |
|----------|-------|
| Type | `int` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `5000` |

## `db.cassandra.speculative_execution_count`

The number of times a query was speculatively executed. Not set or `0` if the query was not executed speculatively.

| Property | Value |
|----------|-------|
| Type | `int` |
| Requirement Level | Recommended |
| Stability | Beta |
| Examples | `0`, `2` |

## `db.cassandra.table`

The name of the primary Cassandra table that the operation is acting upon, including the keyspace name (if applicable).

This mirrors the db.sql.table attribute but references cassandra rather than sql. It is not recommended to attempt any client-side parsing of `db.statement` just to get this property, but it should be set if it is provided by the library being instrumented. If the operation is acting upon an anonymous table, or more than one table, this value MUST NOT be set.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `mytable` |

## `db.connection_string`

The connection string used to connect to the database. It is recommended to remove embedded credentials.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `Server=(localdb)\v11.0;Integrated Security=true;` |

## `db.cosmosdb.client_id`

Unique Cosmos client instance id.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `3ba4827d-4422-483f-b59f-85b74211c11d` |

## `db.cosmosdb.connection_mode`

Cosmos client connection mode.

| Property | Value |
|----------|-------|
| Type | Enum ([see values below](#enum-values)) |
| Requirement Level | Recommended |
| Stability | Stable |

### Enum Values

| Value | Description | Stability |
|-------|-------------|-----------|
| `gateway` | Gateway (HTTP) connections mode | Stable |
| `direct` | Direct connection. | Stable |

## `db.cosmosdb.container`

Cosmos DB container name.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `anystring` |

## `db.cosmosdb.operation_type`

CosmosDB Operation Type.

| Property | Value |
|----------|-------|
| Type | Enum ([see values below](#enum-values)) |
| Requirement Level | Recommended |
| Stability | Stable |

### Enum Values

| Value | Description | Stability |
|-------|-------------|-----------|
| `Invalid` | - | Stable |
| `Create` | - | Stable |
| `Patch` | - | Stable |
| `Read` | - | Stable |
| `ReadFeed` | - | Stable |
| `Delete` | - | Stable |
| `Replace` | - | Stable |
| `Execute` | - | Stable |
| `Query` | - | Stable |
| `Head` | - | Stable |
| `HeadFeed` | - | Stable |
| `Upsert` | - | Stable |
| `Batch` | - | Stable |
| `QueryPlan` | - | Stable |
| `ExecuteJavaScript` | - | Stable |

## `db.cosmosdb.request_charge`

RU consumed for that operation

| Property | Value |
|----------|-------|
| Type | `double` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `46.18`, `1.0` |

## `db.cosmosdb.request_content_length`

Request payload size in bytes

| Property | Value |
|----------|-------|
| Type | `int` |
| Requirement Level | Recommended |
| Stability | Stable |

## `db.cosmosdb.status_code`

Cosmos DB status code.

| Property | Value |
|----------|-------|
| Type | `int` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `200`, `201` |

## `db.cosmosdb.sub_status_code`

Cosmos DB sub status code.

| Property | Value |
|----------|-------|
| Type | `int` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `1000`, `1002` |

## `db.elasticsearch.cluster.name`

Represents the identifier of an Elasticsearch cluster.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `e9106fc68e3044f0b1475b04bf4ffd5f` |

## `db.elasticsearch.node.name`

Represents the human-readable identifier of the node/instance to which a request was routed.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `instance-0000000001` |

## `db.elasticsearch.path_parts`

A dynamic value in the url path.

Many Elasticsearch url paths allow dynamic values. These SHOULD be recorded in span attributes in the format `db.elasticsearch.path_parts.<key>`, where `<key>` is the url path part name. The implementation SHOULD reference the [elasticsearch schema](https://raw.githubusercontent.com/elastic/elasticsearch-specification/main/output/schema/schema.json) in order to map the path part values to their names.

| Property | Value |
|----------|-------|
| Type | `template[string]` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `db.elasticsearch.path_parts.index=test-index`, `db.elasticsearch.path_parts.doc_id=123` |

## `db.instance.id`

An identifier (address, unique name, or any other identifier) of the database instance that is executing queries or mutations on the current connection. This is useful in cases where the database is running in a clustered environment and the instrumentation is able to record the node executing the query. The client may obtain this value in databases like MySQL using queries like `select @@hostname`.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `mysql-e26b99z.example.com` |

## `db.jdbc.driver_classname`

The fully-qualified class name of the [Java Database Connectivity (JDBC)](https://docs.oracle.com/javase/8/docs/technotes/guides/jdbc/) driver used to connect.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `org.postgresql.Driver`, `com.microsoft.sqlserver.jdbc.SQLServerDriver` |

## `db.mongodb.collection`

The MongoDB collection being accessed within the database stated in `db.name`.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `customers`, `products` |

## `db.mssql.instance_name`

The Microsoft SQL Server [instance name](https://docs.microsoft.com/sql/connect/jdbc/building-the-connection-url?view=sql-server-ver15) connecting to. This name is used to determine the port of a named instance.

If setting a `db.mssql.instance_name`, `server.port` is no longer required (but still recommended if non-standard).

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `MSSQLSERVER` |

## `db.name`

This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).

In some SQL databases, the database name to be used is called "schema name". In case there are multiple layers that could be considered for database name (e.g. Oracle instance name and schema name), the database name to be used is the more specific layer (e.g. Oracle schema name).

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `customers`, `main` |

## `db.operation`

The name of the operation being executed, e.g. the [MongoDB command name](https://docs.mongodb.com/manual/reference/command/#database-operations) such as `findAndModify`, or the SQL keyword.

When setting this to an SQL keyword, it is not recommended to attempt any client-side parsing of `db.statement` just to get this property, but it should be set if the operation name is provided by the library being instrumented. If the SQL statement has an ambiguous operation, or performs more than one operation, this value may be omitted.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `findAndModify`, `HMSET`, `SELECT` |

## `db.redis.database_index`

The index of the database being accessed as used in the [`SELECT` command](https://redis.io/commands/select), provided as an integer. To be used instead of the generic `db.name` attribute.

| Property | Value |
|----------|-------|
| Type | `int` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `0`, `1`, `15` |

## `db.sensitive_attribute`

An attribute that contains sensitive information.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Development |
| Examples | `bar` |

## `db.sql.table`

The name of the primary table that the operation is acting upon, including the database name (if applicable).

It is not recommended to attempt any client-side parsing of `db.statement` just to get this property, but it should be set if it is provided by the library being instrumented. If the operation is acting upon an anonymous table, or more than one table, this value MUST NOT be set.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `public.users`, `customers` |

## `db.statement`

The database statement being executed.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `SELECT * FROM wuser_table`, `SET mykey "WuValue"` |

## `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.

| Property | Value |
|----------|-------|
| Type | Enum ([see values below](#enum-values)) |
| Requirement Level | Recommended |
| Stability | Stable |

### Enum Values

| Value | Description | Stability |
|-------|-------------|-----------|
| `other_sql` | Some other SQL database. Fallback only. See notes. | Development |
| `mssql` | Microsoft SQL Server | Stable |
| `mssqlcompact` | Microsoft SQL Server Compact | Development |
| `mysql` | MySQL | Stable |
| `oracle` | Oracle Database | Stable |
| `db2` | IBM Db2 | Stable |
| `postgresql` | PostgreSQL | Stable |
| `redshift` | Amazon Redshift | Stable |
| `hive` | Apache Hive | Stable |
| `cloudscape` | Cloudscape | Stable |
| `hsqldb` | HyperSQL DataBase | Stable |
| `progress` | Progress Database | Stable |
| `maxdb` | SAP MaxDB | Stable |
| `hanadb` | SAP HANA | Stable |
| `ingres` | Ingres | Stable |
| `firstsql` | FirstSQL | Stable |
| `edb` | EnterpriseDB | Stable |
| `cache` | InterSystems Caché | Stable |
| `adabas` | Adabas (Adaptable Database System) | Stable |
| `firebird` | Firebird | Stable |
| `derby` | Apache Derby | Stable |
| `filemaker` | FileMaker | Stable |
| `informix` | Informix | Stable |
| `instantdb` | InstantDB | Stable |
| `interbase` | InterBase | Stable |
| `mariadb` | MariaDB | Stable |
| `netezza` | Netezza | Stable |
| `pervasive` | Pervasive PSQL | Stable |
| `pointbase` | PointBase | Stable |
| `sqlite` | SQLite | Stable |
| `sybase` | Sybase | Stable |
| `teradata` | Teradata | Stable |
| `vertica` | Vertica | Stable |
| `h2` | H2 | Stable |
| `coldfusion` | ColdFusion IMQ | Stable |
| `cassandra` | Apache Cassandra | Stable |
| `hbase` | Apache HBase | Stable |
| `mongodb` | MongoDB | Stable |
| `redis` | Redis | Stable |
| `couchbase` | Couchbase | Stable |
| `couchdb` | CouchDB | Stable |
| `azure.cosmosdb` | Microsoft Azure Cosmos DB | Stable |
| `dynamodb` | Amazon DynamoDB | Stable |
| `neo4j` | Neo4j | Stable |
| `geode` | Apache Geode | Stable |
| `elasticsearch` | Elasticsearch | Stable |
| `memcached` | Memcached | Stable |
| `cockroachdb` | CockroachDB | Stable |
| `opensearch` | OpenSearch | Stable |
| `clickhouse` | ClickHouse | Stable |
| `spanner` | Cloud Spanner | Stable |
| `trino` | Trino | Stable |

## `db.user`

Username for accessing the database.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `readonly_user`, `reporting_user` |

