# DB

<!-- Note: Compared to build-tools, we've removed any-of constraint texts. -->

<!-- semconv db(tag=connection-level) -->
| Attribute  | Type | Description  | Examples  | [Requirement Level](https://opentelemetry.io/docs/specs/semconv/general/attribute-requirement-level/) | Stability |
|---|---|---|---|---|
| db.type | string | Database type. For any SQL database, "sql". For others, the lower-case database category. | `sql`; `cassandra`; `hbase`; `mongodb`; `redis`; `couchbase`; `couchdb` | Required |
| db.connection_string | string | The connection string used to connect to the database. [1] | `Server=(localdb)\v11.0;Integrated Security=true;` | Recommended |
| db.user | string | Username for accessing the database. | `readonly_user`; `reporting_user` | Recommended |
| net.peer.ip | string | Remote address of the peer (dotted decimal for IPv4 or [RFC5952](https://tools.ietf.org/html/rfc5952) for IPv6) | `127.0.0.1` | Recommended |
| net.peer.name | string | Remote hostname or similar, see note below. | `example.com` | Recommended |
| net.peer.port | int | Remote port number. | `80`; `8080`; `443` | Recommended |

**[1]:** It is recommended to remove embedded credentials.

`db.type` has the following list of well-known values. If one of them applies, then the respective value MUST be used; otherwise, a custom value MAY be used.

| Value  | Description | Stability |
|---|---|---|
| `sql` | A SQL database | Experimental |
| `cassandra` | Apache Cassandra | Experimental |
| `hbase` | Apache HBase | Experimental |
| `mongodb` | MongoDB | Experimental |
| `redis` | Redis | Experimental |
| `couchbase` | Couchbase | Experimental |
| `couchdb` | CouchDB | Experimental |
<!-- endsemconv -->
