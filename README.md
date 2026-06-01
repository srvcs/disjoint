# srvcs-disjoint

## Name

| Field | Value |
| --- | --- |
| Service | `srvcs-disjoint` |
| Slug | `disjoint` |
| Repository | `srvcs/disjoint` |
| Package | `srvcs-disjoint` |
| Kind | `orchestrator` |

## Function

sets: do the sets share no elements

## Dependencies

| Dependency | Repository |
| --- | --- |
| `srvcs-intersection` | [srvcs/intersection](https://github.com/srvcs/intersection) |

## API

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/` | Service identity |
| `POST` | `/` | Evaluate the service function |
| `GET` | `/healthz` | Liveness probe |
| `GET` | `/readyz` | Readiness probe |
| `GET` | `/metrics` | Prometheus metrics |
| `GET` | `/openapi.json` | OpenAPI document |

## Inputs

| Name | Type | Required |
| --- | --- | --- |
| `a` | `json[]` | yes |
| `b` | `json[]` | yes |

## Outputs

| Name | Type |
| --- | --- |
| `a` | `json[]` |
| `b` | `json[]` |
| `result` | `boolean` |

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `SRVCS_BIND_ADDR` | `0.0.0.0:8080` | Bind address |
| `SRVCS_ENV` | `development` | Environment label for logs |
| `RUST_LOG` | `info,tower_http=info` | Tracing filter |
| `SRVCS_INTERSECTION_URL` | `` | Base URL for srvcs-intersection |

## Error Behavior

- `422` means the request could not be evaluated for the documented input shape.
- `503` means a required dependency was unavailable or returned an unexpected response.
- Dependency validation errors are forwarded when this service delegates validation.

## Local Checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

See the [srvcs service standard](https://github.com/srvcs/platform/blob/main/STANDARD.md) for the full operational contract.

## Metadata

Machine-readable service metadata lives in `srvcs.yaml`. Keep it aligned with this README when the service contract changes.
