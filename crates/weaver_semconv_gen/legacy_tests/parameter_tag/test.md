# DB

<!-- Note: Compared to build-tools, we've removed any-of constraint texts. -->

<!-- semconv db(tag=connection-level) -->
| Attribute  | Type | Description  | Examples  | [Requirement Level](https://opentelemetry.io/docs/specs/semconv/general/attribute-requirement-level/) | Stability |
|---|---|---|---|---|---|
| db.type | string | Database type. For any SQL database, "sql". For others, the lower-case database category. | `sql`; `cassandra`; `hbase`; `mongodb`; `redis`; `couchbase`; `couchdb` | `Required` | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| db.connection_string | string | The connection string used to connect to the database. [1] | `Server=(localdb)\v11.0;Integrated Security=true;` | `Recommended` | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| db.user | string | Username for accessing the database. | `readonly_user`; `reporting_user` | `Recommended` | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| net.peer.ip | string | Remote address of the peer (dotted decimal for IPv4 or [RFC5952](https://tools.ietf.org/html/rfc5952) for IPv6) | `127.0.0.1` | `Recommended` | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| net.peer.name | string | Remote hostname or similar, see note below. | `example.com` | `Recommended` | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| net.peer.port | int | Remote port number. | `80`; `8080`; `443` | `Recommended` | ![Experimental](https://img.shields.io/badge/-experimental-blue) |

**[1]:** It is recommended to remove embedded credentials.

`db.type` has the following list of well-known values. If one of them applies, then the respective value MUST be used; otherwise, a custom value MAY be used.

| Value  | Description | Stability |
|---|---|---|
| `sql` | A SQL database | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `cassandra` | Apache Cassandra | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `hbase` | Apache HBase | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `mongodb` | MongoDB | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `redis` | Redis | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `couchbase` | Couchbase | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `couchdb` | CouchDB | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
<!-- endsemconv -->
