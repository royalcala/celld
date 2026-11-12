# Limitations

celld v0.6.3 is a beta release with these operational limits. See
[Cloudflare compatibility](cloudflare-compat.md) for the supported services,
APIs, and Wrangler configuration.

## Fleets

- A fleet runs one application. celld has no account service, multi-tenant
  scheduler, or managed ingress.
- A fleet stores its durable state in an S3-compatible bucket, a Google Cloud
  Storage bucket, or an Azure Blob Storage container. Only `celld dev` can use
  the local SQLite object store.
- Ownership balancing counts cells by node weight, not by CPU or memory use.
  It moves only hibernated cells, so a fleet without idle eviction balances
  only the cells that hibernate on their own.

## Networking and security

- celld does not terminate TLS. Terminate public TLS at an ingress proxy, and
  put the internal listener on a private network or an encrypted overlay.
- Peer traffic is plaintext HTTP. The fleet HMAC authenticates tunnel
  establishment and control requests but does not encrypt data, so the network
  must provide confidentiality.
- The fleet bucket controls the fleet. Give its credentials access to one fleet
  only. See [Security](security.md).

## Object storage credentials

- The credential methods differ by provider. See
  [Configure object storage](README.md#configure-object-storage).
- Azure identity works only in the public Azure cloud. A managed identity from
  Azure App Service or Azure Container Apps does not work. Use a workload
  identity or a storage account key there.

## WebSockets

- An outbound Durable Object WebSocket keeps its cell resident. It closes when
  the cell moves to another node, so the application must store the
  connection intent and reconnect.
- A node limits the resident cells and outbound WebSockets.

## Facets

- A facet can start facets of its own, to a total depth of 4 that counts the
  root object. The depth is the length of the facet's name path under its root,
  and starting one past it throws at the `ctx.facets.get()` call.
- A facet can mint a persistent stub with `ctx.restore()` only through a parent
  stub its own root created with `ctx.restore()`. The root's row stores the
  parent handle and the params; reopening the stub replays that chain one level
  at a time, re-minting each parent from its own row. A nested chain therefore
  has to hang from one root object, which is the shape gatekeeper/gadget facets
  use, and that is the supported one. When the durable data belongs to a
  separately deployed script, use its own Durable Object reached through a
  service binding; a persistent-stub chain does not span two roots.

## Platforms

- The installer supplies binaries for Linux x86-64, Linux ARM64, and Apple
  Silicon. Windows is not supported.
