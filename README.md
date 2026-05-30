# srvcs-disjoint

The disjoint-sets predicate of the srvcs.cloud distributed standard library.

Its single concern: **do two sets share no elements?** It does no set logic of
its own. It asks
[`srvcs-intersection`](https://github.com/srvcs/intersection) for the
intersection of the two lists, then reports whether that intersection is empty:

```text
inter  = intersection(a, b).result   # one HTTP call to srvcs-intersection
result = inter is empty
```

So `disjoint({a:[1,2], b:[3,4]}) == true` (no shared elements) and
`disjoint({a:[1,2], b:[2,3]}) == false` (they share `2`).

## API

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/` | Service identity, concern, and dependency list |
| `POST` | `/` | Report whether sets `a` and `b` are disjoint |
| `GET` | `/healthz` `/readyz` `/metrics` `/openapi.json` | srvcs service standard surface |

```sh
curl -s -X POST localhost:8080/ -H 'content-type: application/json' -d '{"a": [1, 2], "b": [3, 4]}'
# {"a":[1,2],"b":[3,4],"result":true}
```

Responses:

- `200 {"a": [...], "b": [...], "result": bool}` — evaluated.
- `422` — an element is not a valid integer, forwarded from `srvcs-intersection`.
- `500` — `srvcs-intersection` returned an unusable response.
- `503` — the `srvcs-intersection` dependency is unavailable.

## Dependencies

- [`srvcs-intersection`](https://github.com/srvcs/intersection)

`srvcs-disjoint` is an orchestrator over a set leaf service. It does **not** call
`srvcs-isnumber` directly: element validation propagates from
`srvcs-intersection`, whose `422`s it forwards unchanged. A single request fans
out to exactly one `disjoint → intersection` call.

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `SRVCS_BIND_ADDR` | `0.0.0.0:8080` | Bind address |
| `SRVCS_INTERSECTION_URL` | `http://127.0.0.1:8081` | Base URL of `srvcs-intersection` |
| `SRVCS_ENV` | `development` | Environment label for logs |
| `RUST_LOG` | `info,tower_http=info` | Tracing filter |

## Local checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Orchestration tests stand up a mock `srvcs-intersection` in-process that
**actually computes** the set intersection from the request body, so the
composition is genuinely exercised (e.g. `disjoint([1,2], [2,3]) == false`). See
[`srvcs/platform`](https://github.com/srvcs/platform) for the shared standard.

> Note: the `cargoHash` in `flake.nix` is inherited from the template and must be
> refreshed with a `nix build` before the Nix gates pass.
