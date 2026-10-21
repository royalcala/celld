# Rolling deploy across a mixed-version fleet

celld capability tokens are fleet-keyed.  A version change that alters the
token layout (for example, the node / isolate-incarnation / facet-route fields
added for cross-node routing) therefore requires a fleet procedure, not a
per-node restart.

## Compatibility window

The current build accepts the previous token layout for **durable handles
only** (`FLAG_RESTORE`).  A durable `ctx.restore` handle keeps working when its
cell moves to an upgraded node: the row is re-resolved there and the isolate
fence is not needed.  Everything else is rejected with a typed error:

| Token minted by | Used on | Result |
| --- | --- | --- |
| previous build, durable handle | current build | accepted (compat path) |
| previous build, transient capability | current build | `forbidden` (`legacy capability has no cross-node fence`) |
| current build | previous build | `forbidden` (previous build rejects the new version) |
| current build | current build | normal |

Transient capabilities are connection-scoped; re-acquire them after the fleet
is upgraded.  The compat path deliberately does not accept transient legacy
tokens, because they carry no node or isolate incarnation and could alias a
reused handle.

## Procedure

1. `celld diagnose --bucket … --endpoint …` on every bucket endpoint; the
   conditional-write check must pass.
2. Upgrade one node at a time.  Before upgrading a node, drain it: stop sending
   it new public traffic and let in-flight calls finish.  Its cells are handed
   off (graceful shutdown) or taken over after the lease TTL.
3. Replace the binary and start the node.  Verify:
   - `celld diagnose …` passes;
   - the public API answers;
   - the cell ownership record in the bucket (`cells/<scope>/own.json`) names a
     single live node for the cells you touched.
4. Do not route traffic to an un-upgraded node for capabilities minted by an
   upgraded node: the old build rejects the new token with `forbidden`.  Drain
   old nodes before new ones start minting into them.
5. After the whole fleet is upgraded, re-establish transient capabilities
   (connections re-acquire them).  Durable hooks/stubs keep their handles
   through the compat window.

## Verified window

`scripts/e2e-mixed-version.sh` (pre-B node A + current node B, then A upgraded)
prints:

- `v1 durable handle stored on A, used on the new node B` — accepted;
- `v1 transient handle from A rejected typed on B` — `forbidden`;
- `v2 transient handle from B rejected by the pre-B node A` — `forbidden`;
- after upgrading A, a fresh callback works (`pong`) and the durable handle
  still reads;
- the cell ownership stays with node B throughout the upgrade.

## Rollback

The previous binary rejects the current token version with a typed
`forbidden`.  Drain the nodes before rolling back, then re-establish transient
capabilities and restart durable handlers; the durable store (restore rows,
DO storage) is not modified by the token version.

## Hard-kill alarm window

A scheduled/alarm cell publishes its wake entry asynchronously.  A node that is
SIGKILLed in the first moments after enabling an alarm can lose that wake
entry, and no successor has a record to fire.  Allow the alarm to ship (a few
seconds) before hard-killing the owner; graceful shutdown drains it.  The
2-node app hook test waits five seconds after `enableHook` for this reason.
