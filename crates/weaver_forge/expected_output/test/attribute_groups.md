# Semantic Convention Attribute Groups

## flatten
- one
- two
- three
## Namespace `db`

### Attributes


#### Attribute `db.cassandra.consistency_level`

The consistency level of the query. Based on consistency values from [CQL](https://docs.datastax.com/en/cassandra-oss/3.0/cassandra/dml/dmlConfigConsistency.html).



- Requirement Level: Recommended
  
- Tag: tech-specific-cassandra
  
- Type: Enum [all, each_quorum, quorum, local_quorum, one, two, three, local_one, any, serial, local_serial]
  
- Stability: Stable
  
  
#### Attribute `db.cassandra.coordinator.dc`

The data center of the coordinating node for a query.



- Requirement Level: Recommended
  
- Tag: tech-specific-cassandra
  
- Type: string
- Examples: us-west-2
  
- Stability: Stable
  
  
#### Attribute `db.cassandra.coordinator.id`

The ID of the coordinating node for a query.



- Requirement Level: Recommended
  
- Tag: tech-specific-cassandra
  
- Type: string
- Examples: be13faa2-8574-4d71-926d-27f16cf8a7af
  
- Stability: Stable
  
  
#### Attribute `db.cassandra.idempotence`

Whether or not the query is idempotent.



- Requirement Level: Recommended
  
- Tag: tech-specific-cassandra
  
- Type: boolean
  
- Stability: Stable
  
  
#### Attribute `db.cassandra.page_size`

The fetch size used for paging, i.e. how many rows will be returned at once.



- Requirement Level: Recommended
  
- Tag: tech-specific-cassandra
  
- Type: int
- Examples: [
    5000,
]
  
- Stability: Stable
  
  
#### Attribute `db.cassandra.speculative_execution_count`

The number of times a query was speculatively executed. Not set or `0` if the query was not executed speculatively.



- Requirement Level: Recommended
  
- Tag: tech-specific-cassandra
  
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
  
- Tag: tech-specific-cassandra
  
- Type: string
- Examples: mytable
  
- Stability: Stable
  
  
#### Attribute `db.connection_string`

The connection string used to connect to the database. It is recommended to remove embedded credentials.



- Requirement Level: Recommended
  
- Tag: db-generic
  
- Type: string
- Examples: Server=(localdb)\v11.0;Integrated Security=true;
  
- Stability: Stable
  
  
#### Attribute `db.cosmosdb.client_id`

Unique Cosmos client instance id.


- Requirement Level: Recommended
  
- Tag: tech-specific-cosmosdb
  
- Type: string
- Examples: 3ba4827d-4422-483f-b59f-85b74211c11d
  
- Stability: Stable
  
  
#### Attribute `db.cosmosdb.connection_mode`

Cosmos client connection mode.


- Requirement Level: Recommended
  
- Tag: tech-specific-cosmosdb
  
- Type: Enum [gateway, direct]
  
- Stability: Stable
  
  
#### Attribute `db.cosmosdb.container`

Cosmos DB container name.


- Requirement Level: Recommended
  
- Tag: tech-specific-cosmosdb
  
- Type: string
- Examples: anystring
  
- Stability: Stable
  
  
#### Attribute `db.cosmosdb.operation_type`

CosmosDB Operation Type.


- Requirement Level: Recommended
  
- Tag: tech-specific-cosmosdb
  
- Type: Enum [Invalid, Create, Patch, Read, ReadFeed, Delete, Replace, Execute, Query, Head, HeadFeed, Upsert, Batch, QueryPlan, ExecuteJavaScript]
  
- Stability: Stable
  
  
#### Attribute `db.cosmosdb.request_charge`

RU consumed for that operation


- Requirement Level: Recommended
  
- Tag: tech-specific-cosmosdb
  
- Type: double
- Examples: [
    46.18,
    1.0,
]
  
- Stability: Stable
  
  
#### Attribute `db.cosmosdb.request_content_length`

Request payload size in bytes


- Requirement Level: Recommended
  
- Tag: tech-specific-cosmosdb
  
- Type: int
  
- Stability: Stable
  
  
#### Attribute `db.cosmosdb.status_code`

Cosmos DB status code.


- Requirement Level: Recommended
  
- Tag: tech-specific-cosmosdb
  
- Type: int
- Examples: [
    200,
    201,
]
  
- Stability: Stable
  
  
#### Attribute `db.cosmosdb.sub_status_code`

Cosmos DB sub status code.


- Requirement Level: Recommended
  
- Tag: tech-specific-cosmosdb
  
- Type: int
- Examples: [
    1000,
    1002,
]
  
- Stability: Stable
  
  
#### Attribute `db.elasticsearch.cluster.name`

Represents the identifier of an Elasticsearch cluster.



- Requirement Level: Recommended
  
- Tag: tech-specific-elasticsearch
  
- Type: string
- Examples: [
    "e9106fc68e3044f0b1475b04bf4ffd5f",
]
  
- Stability: Stable
  
  
#### Attribute `db.elasticsearch.node.name`

Represents the human-readable identifier of the node/instance to which a request was routed.



- Requirement Level: Recommended
  
- Tag: tech-specific-elasticsearch
  
- Type: string
- Examples: [
    "instance-0000000001",
]
  
- Stability: Stable
  
  
#### Attribute `db.elasticsearch.path_parts`

A dynamic value in the url path.



Many Elasticsearch url paths allow dynamic values. These SHOULD be recorded in span attributes in the format `db.elasticsearch.path_parts.<key>`, where `<key>` is the url path part name. The implementation SHOULD reference the [elasticsearch schema](https://raw.githubusercontent.com/elastic/elasticsearch-specification/main/output/schema/schema.json) in order to map the path part values to their names.

- Requirement Level: Recommended
  
- Tag: tech-specific-elasticsearch
  
- Type: template[string]
- Examples: [
    "db.elasticsearch.path_parts.index=test-index",
    "db.elasticsearch.path_parts.doc_id=123",
]
  
- Stability: Stable
  
  
#### Attribute `db.instance.id`

An identifier (address, unique name, or any other identifier) of the database instance that is executing queries or mutations on the current connection. This is useful in cases where the database is running in a clustered environment and the instrumentation is able to record the node executing the query. The client may obtain this value in databases like MySQL using queries like `select @@hostname`.



- Requirement Level: Recommended
  
- Tag: db-generic
  
- Type: string
- Examples: mysql-e26b99z.example.com
  
- Stability: Stable
  
  
#### Attribute `db.jdbc.driver_classname`

The fully-qualified class name of the [Java Database Connectivity (JDBC)](https://docs.oracle.com/javase/8/docs/technotes/guides/jdbc/) driver used to connect.



- Requirement Level: Recommended
  
- Tag: tech-specific-jdbc
  
- Type: string
- Examples: [
    "org.postgresql.Driver",
    "com.microsoft.sqlserver.jdbc.SQLServerDriver",
]
  
- Stability: Stable
  
  
#### Attribute `db.mongodb.collection`

The MongoDB collection being accessed within the database stated in `db.name`.



- Requirement Level: Recommended
  
- Tag: tech-specific-mongodb
  
- Type: string
- Examples: [
    "customers",
    "products",
]
  
- Stability: Stable
  
  
#### Attribute `db.mssql.instance_name`

The Microsoft SQL Server [instance name](https://docs.microsoft.com/sql/connect/jdbc/building-the-connection-url?view=sql-server-ver15) connecting to. This name is used to determine the port of a named instance.



If setting a `db.mssql.instance_name`, `server.port` is no longer required (but still recommended if non-standard).

- Requirement Level: Recommended
  
- Tag: tech-specific-mssql
  
- Type: string
- Examples: MSSQLSERVER
  
- Stability: Stable
  
  
#### Attribute `db.name`

This attribute is used to report the name of the database being accessed. For commands that switch the database, this should be set to the target database (even if the command fails).



In some SQL databases, the database name to be used is called "schema name". In case there are multiple layers that could be considered for database name (e.g. Oracle instance name and schema name), the database name to be used is the more specific layer (e.g. Oracle schema name).

- Requirement Level: Recommended
  
- Tag: db-generic
  
- Type: string
- Examples: [
    "customers",
    "main",
]
  
- Stability: Stable
  
  
#### Attribute `db.operation`

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
  
- Stability: Stable
  
  
#### Attribute `db.redis.database_index`

The index of the database being accessed as used in the [`SELECT` command](https://redis.io/commands/select), provided as an integer. To be used instead of the generic `db.name` attribute.



- Requirement Level: Recommended
  
- Tag: tech-specific-redis
  
- Type: int
- Examples: [
    0,
    1,
    15,
]
  
- Stability: Stable
  
  
#### Attribute `db.sql.table`

The name of the primary table that the operation is acting upon, including the database name (if applicable).


It is not recommended to attempt any client-side parsing of `db.statement` just to get this property, but it should be set if it is provided by the library being instrumented. If the operation is acting upon an anonymous table, or more than one table, this value MUST NOT be set.

- Requirement Level: Recommended
  
- Tag: tech-specific-sql
  
- Type: string
- Examples: [
    "public.users",
    "customers",
]
  
- Stability: Stable
  
  
#### Attribute `db.statement`

The database statement being executed.



- Requirement Level: Recommended
  
- Tag: db-generic
  
- Type: string
- Examples: [
    "SELECT * FROM wuser_table",
    "SET mykey \"WuValue\"",
]
  
- Stability: Stable
  
  
#### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Recommended
  
- Tag: db-generic
  
- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]
  
- Stability: Stable
  
  
#### Attribute `db.user`

Username for accessing the database.



- Requirement Level: Recommended
  
- Tag: db-generic
  
- Type: string
- Examples: [
    "readonly_user",
    "reporting_user",
]
  
- Stability: Stable
  
  
  
## Namespace `exception`

### Attributes


#### Attribute `exception.escaped`

SHOULD be set to true if the exception event is recorded at a point where it is known that the exception is escaping the scope of the span.



An exception is considered to have escaped (or left) the scope of a span,
if that span is ended while the exception is still logically "in flight".
This may be actually "in flight" in some languages (e.g. if the exception
is passed to a Context manager's `__exit__` method in Python) but will
usually be caught at the point of recording the exception in most languages.

It is usually not possible to determine at the point where an exception is thrown
whether it will escape the scope of a span.
However, it is trivial to know that an exception
will escape, if one checks for an active exception just before ending the span,
as done in the [example for recording span exceptions](https://opentelemetry.io/docs/specs/semconv/exceptions/exceptions-spans/#recording-an-exception).

It follows that an exception may still escape the scope of the span
even if the `exception.escaped` attribute was not set or set to false,
since the event might have been recorded at a time where it was not
clear whether the exception will escape.

- Requirement Level: Recommended
  
- Type: boolean
  
- Stability: Stable
  
  
#### Attribute `exception.message`

The exception message.


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "Division by zero",
    "Can't convert 'int' object to str implicitly",
]
  
- Stability: Stable
  
  
#### Attribute `exception.stacktrace`

A stacktrace as a string in the natural representation for the language runtime. The representation is to be determined and documented by each language SIG.



- Requirement Level: Recommended
  
- Type: string
- Examples: Exception in thread "main" java.lang.RuntimeException: Test exception\n at com.example.GenerateTrace.methodB(GenerateTrace.java:13)\n at com.example.GenerateTrace.methodA(GenerateTrace.java:9)\n at com.example.GenerateTrace.main(GenerateTrace.java:5)
  
- Stability: Stable
  
  
#### Attribute `exception.type`

The type of the exception (its fully-qualified class name, if applicable). The dynamic type of the exception should be preferred over the static type in languages that support it.



- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "java.net.ConnectException",
    "OSError",
]
  
- Stability: Stable
  
  
  
## Namespace `http`

### Attributes


#### Attribute `http.request.body.size`

The size of the request payload body in bytes. This is the number of bytes transferred excluding headers and is often, but not always, present as the [Content-Length](https://www.rfc-editor.org/rfc/rfc9110.html#field.content-length) header. For requests using transport encoding, this should be the compressed size.



- Requirement Level: Recommended
  
- Type: int
- Examples: 3495
  
- Stability: Development
  
  
#### Attribute `http.request.header`

HTTP request headers, `<key>` being the normalized HTTP Header name (lowercase), the value being the header values.



Instrumentations SHOULD require an explicit configuration of which headers are to be captured. Including all request headers can be a security risk - explicit configuration helps avoid leaking sensitive information.
The `User-Agent` header is already captured in the `user_agent.original` attribute. Users MAY explicitly configure instrumentations to capture them even though it is not recommended.
The attribute value MUST consist of either multiple header values as an array of strings or a single-item array containing a possibly comma-concatenated string, depending on the way the HTTP library provides access to headers.

- Requirement Level: Recommended
  
- Type: template[string[]]
- Examples: [
    "http.request.header.content-type=[\"application/json\"]",
    "http.request.header.x-forwarded-for=[\"1.2.3.4\", \"1.2.3.5\"]",
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

- Requirement Level: Recommended
  
- Type: Enum [CONNECT, DELETE, GET, HEAD, OPTIONS, PATCH, POST, PUT, TRACE, _OTHER]
- Examples: [
    "GET",
    "POST",
    "HEAD",
]
  
- Stability: Stable
  
  
#### Attribute `http.request.method_original`

Original HTTP method sent by the client in the request line.


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "GeT",
    "ACL",
    "foo",
]
  
- Stability: Stable
  
  
#### Attribute `http.request.resend_count`

The ordinal number of request resending attempt (for any reason, including redirects).



The resend count SHOULD be updated each time an HTTP request gets resent by the client, regardless of what was the cause of the resending (e.g. redirection, authorization failure, 503 Server Unavailable, network issues, or any other).

- Requirement Level: Recommended
  
- Type: int
- Examples: 3
  
- Stability: Stable
  
  
#### Attribute `http.response.body.size`

The size of the response payload body in bytes. This is the number of bytes transferred excluding headers and is often, but not always, present as the [Content-Length](https://www.rfc-editor.org/rfc/rfc9110.html#field.content-length) header. For requests using transport encoding, this should be the compressed size.



- Requirement Level: Recommended
  
- Type: int
- Examples: 3495
  
- Stability: Development
  
  
#### Attribute `http.response.header`

HTTP response headers, `<key>` being the normalized HTTP Header name (lowercase), the value being the header values.



Instrumentations SHOULD require an explicit configuration of which headers are to be captured. Including all response headers can be a security risk - explicit configuration helps avoid leaking sensitive information.
Users MAY explicitly configure instrumentations to capture them even though it is not recommended.
The attribute value MUST consist of either multiple header values as an array of strings or a single-item array containing a possibly comma-concatenated string, depending on the way the HTTP library provides access to headers.

- Requirement Level: Recommended
  
- Type: template[string[]]
- Examples: [
    "http.response.header.content-type=[\"application/json\"]",
    "http.response.header.my-custom-header=[\"abc\", \"def\"]",
]
  
- Stability: Stable
  
  
#### Attribute `http.response.status_code`

[HTTP response status code](https://tools.ietf.org/html/rfc7231#section-6).


- Requirement Level: Recommended
  
- Type: int
- Examples: [
    200,
]
  
- Stability: Stable
  
  
#### Attribute `http.route`

The matched route, that is, the path template in the format used by the respective server framework.



MUST NOT be populated when this is not supported by the HTTP server framework as the route attribute should have low-cardinality and the URI path can NOT substitute it.
SHOULD include the [application root](/docs/http/http-spans.md#http-server-definitions) if there is one.

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "/users/:userID?",
    "{controller}/{action}/{id?}",
]
  
- Stability: Stable
  
  
  
## Namespace `network`

### Attributes


#### Attribute `network.carrier.icc`

The ISO 3166-1 alpha-2 2-character country code associated with the mobile carrier network.


- Requirement Level: Recommended
  
- Type: string
- Examples: DE
  
- Stability: Stable
  
  
#### Attribute `network.carrier.mcc`

The mobile carrier country code.


- Requirement Level: Recommended
  
- Type: string
- Examples: 310
  
- Stability: Stable
  
  
#### Attribute `network.carrier.mnc`

The mobile carrier network code.


- Requirement Level: Recommended
  
- Type: string
- Examples: 001
  
- Stability: Stable
  
  
#### Attribute `network.carrier.name`

The name of the mobile carrier.


- Requirement Level: Recommended
  
- Type: string
- Examples: sprint
  
- Stability: Stable
  
  
#### Attribute `network.connection.subtype`

This describes more details regarding the connection.type. It may be the type of cell technology connection, but it could be used for describing details about a wifi connection.


- Requirement Level: Recommended
  
- Type: Enum [gprs, edge, umts, cdma, evdo_0, evdo_a, cdma2000_1xrtt, hsdpa, hsupa, hspa, iden, evdo_b, lte, ehrpd, hspap, gsm, td_scdma, iwlan, nr, nrnsa, lte_ca]
- Examples: LTE
  
- Stability: Stable
  
  
#### Attribute `network.connection.type`

The internet connection type.


- Requirement Level: Recommended
  
- Type: Enum [wifi, wired, cell, unavailable, unknown]
- Examples: wifi
  
- Stability: Stable
  
  
#### Attribute `network.io.direction`

The network IO operation direction.


- Requirement Level: Recommended
  
- Type: Enum [transmit, receive]
- Examples: [
    "transmit",
]
  
- Stability: Stable
  
  
#### Attribute `network.local.address`

Local address of the network connection - IP address or Unix domain socket name.


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "10.1.2.80",
    "/tmp/my.sock",
]
  
- Stability: Stable
  
  
#### Attribute `network.local.port`

Local port number of the network connection.


- Requirement Level: Recommended
  
- Type: int
- Examples: [
    65123,
]
  
- Stability: Stable
  
  
#### Attribute `network.peer.address`

Peer address of the network connection - IP address or Unix domain socket name.


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "10.1.2.80",
    "/tmp/my.sock",
]
  
- Stability: Stable
  
  
#### Attribute `network.peer.port`

Peer port number of the network connection.


- Requirement Level: Recommended
  
- Type: int
- Examples: [
    65123,
]
  
- Stability: Stable
  
  
#### Attribute `network.protocol.name`

[OSI application layer](https://osi-model.com/application-layer/) or non-OSI equivalent.


The value SHOULD be normalized to lowercase.

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "amqp",
    "http",
    "mqtt",
]
  
- Stability: Stable
  
  
#### Attribute `network.protocol.version`

Version of the protocol specified in `network.protocol.name`.


`network.protocol.version` refers to the version of the protocol used and might be different from the protocol client's version. If the HTTP client has a version of `0.27.2`, but sends HTTP version `1.1`, this attribute should be set to `1.1`.

- Requirement Level: Recommended
  
- Type: string
- Examples: 3.1.1
  
- Stability: Stable
  
  
#### Attribute `network.transport`

[OSI transport layer](https://osi-model.com/transport-layer/) or [inter-process communication method](https://wikipedia.org/wiki/Inter-process_communication).



The value SHOULD be normalized to lowercase.

Consider always setting the transport when setting a port number, since
a port number is ambiguous without knowing the transport. For example
different processes could be listening on TCP port 12345 and UDP port 12345.

- Requirement Level: Recommended
  
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
  
- Type: Enum [ipv4, ipv6]
- Examples: [
    "ipv4",
    "ipv6",
]
  
- Stability: Stable
  
  
  
## Namespace `url`

### Attributes


#### Attribute `url.fragment`

The [URI fragment](https://www.rfc-editor.org/rfc/rfc3986#section-3.5) component


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "SemConv",
]
  
- Stability: Stable
  
  
#### Attribute `url.full`

Absolute URL describing a network resource according to [RFC3986](https://www.rfc-editor.org/rfc/rfc3986)


For network calls, URL usually has `scheme://host[:port][path][?query][#fragment]` format, where the fragment is not transmitted over HTTP, but if it is known, it SHOULD be included nevertheless.
`url.full` MUST NOT contain credentials passed via URL in form of `https://username:password@www.example.com/`. In such case username and password SHOULD be redacted and attribute's value SHOULD be `https://REDACTED:REDACTED@www.example.com/`.
`url.full` SHOULD capture the absolute URL when it is available (or can be reconstructed) and SHOULD NOT be validated or modified except for sanitizing purposes.

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "https://www.foo.bar/search?q=OpenTelemetry#SemConv",
    "//localhost",
]
  
- Stability: Stable
  
  
#### Attribute `url.path`

The [URI path](https://www.rfc-editor.org/rfc/rfc3986#section-3.3) component


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "/search",
]
  
- Stability: Stable
  
  
#### Attribute `url.query`

The [URI query](https://www.rfc-editor.org/rfc/rfc3986#section-3.4) component


Sensitive content provided in query string SHOULD be scrubbed when instrumentations can identify it.

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "q=OpenTelemetry",
]
  
- Stability: Stable
  
  
#### Attribute `url.scheme`

The [URI scheme](https://www.rfc-editor.org/rfc/rfc3986#section-3.1) component identifying the used protocol.


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "https",
    "ftp",
    "telnet",
]
  
- Stability: Stable
  
  
  
## Namespace `user_agent`

### Attributes


#### Attribute `user_agent.original`

Value of the [HTTP User-Agent](https://www.rfc-editor.org/rfc/rfc9110.html#field.user-agent) header sent by the client.



- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "CERN-LineMode/2.15 libwww/2.17b3",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 14_7_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.1.2 Mobile/15E148 Safari/604.1",
]
  
- Stability: Stable
  
  
  