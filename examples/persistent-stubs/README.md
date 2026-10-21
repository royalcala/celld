# Persistent stubs (`ctx.restore`)

This fixture is runtime evidence for the durable `ctx.restore` work. It has no
celld-specific code: the only API it uses is `cloudflare:workers`.

A facet has no published cell route, so its persistent stub is minted by its
root (a normal cell route). The root stores `{parent, params}` in its
`_cf_RESTORE` table. On replay the root re-runs `[restore]` (which reopens the
facet), and the facet re-runs its own `[restore]`.

## Run

```sh
celld dev examples/persistent-stubs/wrangler.jsonc --port 18103 --no-watch --clean
```

```sh
B=http://127.0.0.1:18103
curl $B/make      # hello, world!
curl $B/store     # stored, one!   (stored in the facet's own storage; parent disposed)
curl $B/stored    # stored, two!   (works while the process lives)
```

Restart without `--clean` (a process restart evicts every cell):

```sh
celld dev examples/persistent-stubs/wrangler.jsonc --port 18103 --no-watch
curl $B/stored    # stored, two!
```

That last call is the durable path: the capability key is derived from the
persisted peer secret, the root row is still in `_cf_RESTORE`, the root reopens
the facet, and the facet restores the greeter from its own storage.

`curl $B/dispose` mints a temporary greeter, disposes it, and the child
`_cf_RESTORE` row is deleted. The parent row survives because the stored child
holds a reference to it.
