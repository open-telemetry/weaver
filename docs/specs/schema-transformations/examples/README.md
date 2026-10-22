# Semantic convention transformations examples

<!-- disable markdownlint requirement for tables to be aligned -->
<!-- markdownlint-disable-file MD060 -->

> [!WARNING]
> 
> Demo only. 
>
> Experimental. The file format is a draft ("transform/2.0"). Nothing
> consumes these files yet, apart from a query-rewrite prototype that was used
> to validate them.

Examples for the [schema transformations design](../schema-transformations.md): the
[migration guides](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/) and the
[gRPC compatibility guide](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/compatibility/grpc.md)
as transformation sets.

## Transformation files

Each guide has one file per direction. The reverse direction is written on its own, not inverted:
a change that can be bridged one way often can't be bridged the other way.

| Area | File | Source → target | Guide |
|---|---|---|---|
| Code attributes | [`code/1.29.0_to_1.33.0.yaml`](code/1.29.0_to_1.33.0.yaml) | 1.29.0 → 1.33.0 | [code-attrs-migration](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/code-attrs-migration.md) |
| | [`code/1.33.0_to_1.29.0.yaml`](code/1.33.0_to_1.29.0.yaml) | 1.33.0 → 1.29.0 | |
| Database | [`db/1.24.0_to_1.33.0.yaml`](db/1.24.0_to_1.33.0.yaml) | 1.24.0 → 1.33.0 | [db-migration](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/db-migration.md) |
| | [`db/1.33.0_to_1.24.0.yaml`](db/1.33.0_to_1.24.0.yaml) | 1.33.0 → 1.24.0 | |
| HTTP | [`http/1.20.0_to_1.23.1.yaml`](http/1.20.0_to_1.23.1.yaml) | 1.20.0 → 1.23.1 | [http-migration](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/http-migration.md) |
| | [`http/1.23.1_to_1.20.0.yaml`](http/1.23.1_to_1.20.0.yaml) | 1.23.1 → 1.20.0 | |
| Kubernetes | [`k8s/collector-1.18.0_to_1.44.0.yaml`](k8s/collector-1.18.0_to_1.44.0.yaml) | Collector (1.18.0) → 1.44.0 | [k8s-migration](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/k8s-migration.md) |
| | [`k8s/1.44.0_to_collector-1.18.0.yaml`](k8s/1.44.0_to_collector-1.18.0.yaml) | 1.44.0 → Collector (1.18.0) | |
| RPC | [`rpc/1.37.0_to_1.45.0.yaml`](rpc/1.37.0_to_1.45.0.yaml) | 1.37.0 → "1.45.0" | [rpc-migration](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/rpc-migration.md) |
| | [`rpc/1.45.0_to_1.37.0.yaml`](rpc/1.45.0_to_1.37.0.yaml) | "1.45.0" → 1.37.0 | |
| gRPC (native) | [`grpc/grpc-42.42.0_to_1.45.0.yaml`](grpc/grpc-42.42.0_to_1.45.0.yaml) | gRPC A66/A72 → "1.45.0" | [compatibility/grpc](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/compatibility/grpc.md) |
| | [`grpc/1.45.0_to_grpc-42.42.0.yaml`](grpc/1.45.0_to_grpc-42.42.0.yaml) | "1.45.0" → gRPC A66/A72 | |
| GenAI | [`gen-ai/1.41.0_to_gen-ai-dev-1.42.0-dev.yaml`](gen-ai/1.41.0_to_gen-ai-dev-1.42.0-dev.yaml) | 1.41.0 → gen-ai-dev 1.42.0-dev | [GenAI migration](https://github.com/lmolkova/semantic-conventions-genai/blob/1780533536101fee409056c8ff24ed99ab861f0a/docs/gen-ai/non-normative/migration.md) |
| | [`gen-ai/gen-ai-dev-1.42.0-dev_to_1.41.0.yaml`](gen-ai/gen-ai-dev-1.42.0-dev_to_1.41.0.yaml) | gen-ai-dev 1.42.0-dev → 1.41.0 | |
