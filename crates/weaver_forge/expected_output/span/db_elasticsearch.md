## Group `db.elasticsearch` (span)

### Brief

Call-level attributes for Elasticsearch



Prefix: 
Kind: none

### Attributes


#### Attribute `db.system`

An identifier for the database management system (DBMS) product being used. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Tag: connection-level
  
- Type: Enum [other_sql, mssql, mssqlcompact, mysql, oracle, db2, postgresql, redshift, hive, cloudscape, hsqldb, progress, maxdb, hanadb, ingres, firstsql, edb, cache, adabas, firebird, derby, filemaker, informix, instantdb, interbase, mariadb, netezza, pervasive, pointbase, sqlite, sybase, teradata, vertica, h2, coldfusion, cassandra, hbase, mongodb, redis, couchbase, couchdb, cosmosdb, dynamodb, neo4j, geode, elasticsearch, memcached, cockroachdb, opensearch, clickhouse, spanner, trino]
  
  
#### Attribute `db.connection_string`

The connection string used to connect to the database. It is recommended to remove embedded credentials.



- Requirement Level: Recommended
  
- Tag: connection-level
  
- Type: string
- Examples: Server=(localdb)\v11.0;Integrated Security=true;
  
  
#### Attribute `db.user`

Username for accessing the database.



- Requirement Level: Recommended
  
- Tag: connection-level
  
- Type: string
- Examples: [
    "readonly_user",
    "reporting_user",
]
  
  
#### Attribute `db.jdbc.driver_classname`

The fully-qualified class name of the [Java Database Connectivity (JDBC)](https://docs.oracle.com/javase/8/docs/technotes/guides/jdbc/) driver used to connect.



- Requirement Level: Recommended
  
- Tag: connection-level-tech-specific
  
- Type: string
- Examples: [
    "org.postgresql.Driver",
    "com.microsoft.sqlserver.jdbc.SQLServerDriver",
]
  
  
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
  
  
#### Attribute `db.elasticsearch.cluster.name`

Represents the identifier of an Elasticsearch cluster.



- Requirement Level: Optional
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "e9106fc68e3044f0b1475b04bf4ffd5f",
]
  
  
#### Attribute `db.elasticsearch.node.name`

Represents the human-readable identifier of the node/instance to which a request was routed.



- Requirement Level: Optional
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "instance-0000000001",
]
  
  
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
  
  
#### Attribute `db.statement`

The request body for a [search-type query](https://www.elastic.co/guide/en/elasticsearch/reference/current/search.html), as a json string.


- Requirement Level: Optional
  
- Tag: call-level-tech-specific
  
- Type: string
- Examples: [
    "\"{\\\"query\\\":{\\\"term\\\":{\\\"user.id\\\":\\\"kimchy\\\"}}}\"",
]
  
  
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
  
  