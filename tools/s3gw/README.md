# us-s3gw

A minimal S3-compatible object store for a `celld` fleet bucket, **owned by us**.

### celld integration

This tree is vendored at `tools/s3gw` and excluded from the celld workspace.
We added one change on top of the fork: metadata and internal-info filenames
hash the key instead of base64-encoding the full key, because celld facet-log
keys exceed `NAME_MAX` (the old naming failed with `File name too long` when a
facet log crossed a node boundary). Object data layout is unchanged. Buckets
created with the old metadata naming do not have migrated attributes; the
fleet tests start from fresh buckets.

```sh
cargo build --release --manifest-path tools/s3gw/Cargo.toml
CELLD_BIN=target/debug/celld S3GW_BIN=tools/s3gw/target/release/us-s3gw \
  bash tools/s3gw/scripts/accept.sh   # celld diagnose
CELLD_BIN=target/debug/celld S3GW_BIN=tools/s3gw/target/release/us-s3gw \
  bash tools/s3gw/scripts/race.sh     # concurrent If-None-Match CAS
```

The CI `.github/workflows/ci.yml` two-node job builds this binary, runs both
gates, then serves the `scripts/e2e-2node.sh` fleet matrix from it.

## Why this exists

`celld` (`denoland/celld`) assigns **ownership of a cell** through a **conditional write to the
fleet bucket** (`If-Match` / `If-None-Match`). Its docs are explicit that a store which *accepts*
those headers and does not *enforce* them "fails late and silently", and the failure mode is
**two nodes owning one cell**. It ships `celld diagnose` as the only real detector.

We measured the self-hostable options (2026-09-27):

| store | `celld diagnose` |
|---|---|
| SeaweedFS | ok |
| RustFS | ok (single endpoint) |
| **Garage** | **fail** — accepts the headers, does not enforce them |
| MinIO | not downloadable (archived, `dl.min.io` → 410) |
| Ceph RGW, Zenko, Cubbit, B2, Hetzner, DO Spaces, Wasabi | no |

Forking a full store means inheriting a huge tree (SeaweedFS ≈ 769k Go lines, with PITR/EC-repair/
bitrot behind a paywall; RustFS ≈ 1.4M Rust lines, single vendor). `s3s-fs` is the opposite: ~2k
lines, Apache-2.0, a complete S3 surface — **but its conditional write is check-then-act with no
lock**, so it has the same defect.

So: vendor the small thing and fix the one thing that matters.

## What we changed

One change, in two places (`put_object`, `complete_multipart_upload`): a conditional write holds a
process-wide write lock **across the check and the write**, so two concurrent conditional PUTs
cannot both pass. Unconditional writes skip the lock — they carry no precondition to race. This is
the bulk path (LTX segments are unconditional PUTs), so only the tiny control-plane writes serialize.

Measured with 24 concurrent `If-None-Match: *` PUTs of the same key:

| | winners per round | verdict |
|---|---|---|
| with the lock (default) | 1, 1, 1, 1, 1 | atomic CAS |
| without it (`US_S3GW_NO_CAS_LOCK=1`) | 3, 3, 2, 24, 7 | **up to 24 winners** |

`US_S3GW_NO_CAS_LOCK=1` exists only so the concurrency test can show the failure the lock prevents.

## Run

```sh
cargo build --release
./target/release/us-s3gw --host 127.0.0.1 --port 9020 \
  --access-key <key> --secret-key <secret> /var/lib/us-s3gw
```

A bucket is a directory under the root; it is created on first write (or by `CreateBucket`, or
simply `mkdir <root>/<bucket>`). `celld diagnose` **lists the bucket before probing it**, so the
bucket directory must exist first.

## Verification (CI)

`just` recipes (or run the scripts directly):

| gate | command | asserts |
|---|---|---|
| formatting | `just fmt` | `cargo fmt --check` clean |
| lint | `just lint` | `cargo clippy --all-targets -- -D warnings` clean |
| build | `just build` | release binary |
| **acceptance** | `just accept` (`scripts/accept.sh`) | `celld diagnose` against a freshly started store **prints `ok bucket conditional write`**; anything else exits 1 |
| **concurrency** | `just race` (`scripts/race.sh`) | 24 concurrent `If-None-Match: *` PUTs → **exactly 1 winner**, 5/5 rounds |
| everything | `just ci` | fmt + lint + build + accept + race |

```sh
just ci                      # the full gate
CELLD_BIN=/path/to/celld just accept   # override celld discovery
S3GW_PORT=9021 just accept    # different port (S3GW_BUCKET/S3GW_ACCESS_KEY/… also overridable)
```

`scripts/accept.sh` finds `celld` via `$CELLD_BIN`, then `PATH`, then
`celld/target/{release,debug}/celld` walking up from the script (works from the repo or a
worktree). It starts the store on a throwaway root, exports `AWS_ACCESS_KEY_ID` /
`AWS_SECRET_ACCESS_KEY`, runs `celld diagnose --bucket s3://… --endpoint http://127.0.0.1:9020
--region us-east-1`, prints the output, and fails unless the literal line appears:

```
ok bucket conditional write: create, reject-create, update, reject-stale
```

(Verified both ways: real `celld diagnose` → PASS/exit 0; a stub that prints
`fail bucket conditional write: …` → FAIL/exit 1.)

The concurrency gate is reproducible on its own:

```sh
just race                                  # 5 rounds × 24 PUTs, asserts 1 winner/round
US_S3GW_NO_CAS_LOCK=1 scripts/race.sh --report-only   # demo: lock OFF, reports winners (no assertion)
```

## Multipart and the bulk path (measured — T42, 2026-10-02)

`celld`'s LTX client (`crates/ltx/src/client/object_store.rs`) uploads with a **single PUT below
5 MiB** (`MULTIPART_THRESHOLD`) and switches to **multipart with 5 MiB parts at ≥ 5 MiB**. Whether
that matters at *our* sizes we measured instead of guessed, two ways:

**1. What a real celld node stores — `scripts/live-sizes.sh` (or `just live`).**
Starts us-s3gw, `celld deploy`s `scripts/live-app`, runs a **production celld node** against the
store, fills an ~8 MiB cell and keeps it dirty until the node's L1 compaction fires (default
triggers: 256 txids or 32 MiB pending, `CELLD_LTX_COMPACTION_MIN_TXIDS`/`_MIN_MB`; input bound
64 MiB). Two consecutive runs:

```
L1 compaction rounds:  min_txid=1 max_txid=148  input_bytes=33575883  output_bytes=7952103
LTX census:  count=385 min=125949 median=130396 max=7952103
             bands: <1KiB=0  1K..600K=378  600K..5M=6  >=5M=1
multipart etag served by us-s3gw: "62b76b026fd2cc41068856207be0dd23-2"
PASS: real celld stored a >=5 MiB object through us-s3gw multipart
```

An earlier run with a small (256 KiB) cell is the other half of the picture: L0 segments of
708 B–266,917 B and an L1 compaction with **40,615,719 B of input that collapsed to a 266,917 B
output** — LTX compaction merges page diffs, so an L1 object is about the **cell DB size**, not the
write volume. Small cells never approach 5 MiB; a cell whose DB passes ~5 MiB does, on both the L0
boundary-snapshot path (measured 4,226,328 B and 4,739,423 B snapshots just under the line) and the
L1 path (measured 7,952,103 / 8,216,890 B objects **over** the line).

**2. What the store itself can do — `scripts/bulk.sh` (or `just bulk`).**
Signed, round-trip-verified (sha256 + ranged GET + ETag shape), localhost, Python client
timings incl. signing:

| operation | size | time | ETag | verify |
|---|---|---|---|---|
| single PUT | 524,288 B (L0 WAL capture cap) | 0.035 s | plain MD5 | ok |
| single PUT | 4,194,304 B (deploy-bundle scale) | 0.199 s | plain MD5 | ok |
| multipart 3×5 MiB | 12,582,912 B | 0.754 s | `…-3` | ok |
| multipart 13×5 MiB | 67,108,864 B (compaction input ceiling) | 5.709 s | `…-13` | ok |

**Verdict:** the bulk path **does** need multipart at our sizes as soon as a cell's DB passes
~5 MiB, and celld switches to it automatically — us-s3gw already implements it (upstream `s3s-fs`
`create/upload/complete_multipart_upload` + our CAS lock on `complete_multipart_upload`), and both
the synthetic and the live measurements above verify it end-to-end. No code change required.
Everything below the threshold (the common case for small cells and deploy bundles) stays on the
single-PUT path. Deploy (`celld deploy`), node logs, and wake records are always single PUTs;
measured deploy objects: 1,154–1,421 B for the probe app, 3,418,764 B for a real app bundle — all
below the threshold.

## Deployment (F0): systemd service

The supported way to run this is a **service**, not a shell command: units, an env file with a
**dedicated data root** and credentials outside the repo, `Restart=always`, and boot start.
See [`deploy/README.md`](deploy/README.md) for install steps and
[`1us-infra/docs/fleet-storage-runbook.md`](../infra/docs/fleet-storage-runbook.md) for
start/stop, **backup/restore** (mirror + `rsync --link-dest` snapshots, restore **proven**),
disk/RAID1 and **failover**.

```sh
just fmt lint build        # code gates
S3GW_ENDPOINT=http://127.0.0.1:9020 just accept   # diagnose against the RUNNING service
S3GW_ENDPOINT=http://127.0.0.1:9020 just race     # 24 concurrent CAS writers -> 1 winner
```

With `S3GW_ENDPOINT` set the gates verify an already-running store (the service) instead of
starting a private one; credentials come from `S3GW_ACCESS_KEY`/`S3GW_SECRET_KEY`.

## Wiring a celld fleet to this store

```sh
# 1. store (one process per root — see limits); in F0 this is the systemd service
./target/release/us-s3gw --host 0.0.0.0 --port 9020 \
  --access-key "$S3_ACCESS_KEY" --secret-key "$S3_SECRET_KEY" /var/lib/us-s3gw
mkdir -p /var/lib/us-s3gw/fleet        # or CreateBucket; bucket must exist before diagnose

# 2. acceptance (must print `ok bucket conditional write`)
AWS_ACCESS_KEY_ID=$S3_ACCESS_KEY AWS_SECRET_ACCESS_KEY=$S3_SECRET_KEY AWS_EC2_METADATA_DISABLED=true \
celld diagnose --bucket s3://fleet --endpoint http://store.host:9020 --region us-east-1 --listen 127.0.0.1:0

# 3. deploy an app
AWS_ACCESS_KEY_ID=… AWS_SECRET_ACCESS_KEY=… AWS_EC2_METADATA_DISABLED=true \
celld deploy ./my-app --bucket s3://fleet --endpoint http://store.host:9020

# 4. run a node (N nodes may share one store)
AWS_ACCESS_KEY_ID=… AWS_SECRET_ACCESS_KEY=… AWS_EC2_METADATA_DISABLED=true \
celld --bucket s3://fleet --endpoint http://store.host:9020 --listen 127.0.0.1:8080
```

`AWS_ENDPOINT_URL` works as an alternative to `--endpoint` (the standard chain also reads
`CELLD_BUCKET` / `S3_ENDPOINT`); region comes from `--region`, `AWS_REGION` or `AWS_DEFAULT_REGION`
(default `us-east-1`). The credentials are the store's `--access-key`/`--secret-key` pair — keep
them in the environment or a `.env` file, never in git. Export
`AWS_EC2_METADATA_DISABLED=true` on hosts without instance metadata: otherwise celld's credential
chain probes `169.254.169.254` first and every command pays (or fails on) that timeout.

## Fleet-storage phases (trigger-driven)

From the spec (`1us-v2/docs/superpowers/specs/2026-10-02-fleet-storage-design.md`, decision D4):
each phase starts on a **trigger**, never by default. Durability is delegated to the **substrate**;
no EC/quorum of our own, and **no MinIO fork** (D3: AGPLv3 + archived upstream).

| phase | runs when | what | cost |
|---|---|---|---|
| **F0 — durable** (now) | already | systemd service + dedicated disk + RAID1 (or mirror dir) + snapshots outside the host + **proven restore** | no new code |
| **F1 — HA** | *if a node outage matters* — **proven on real hardware (T87)** | filesystem replication (`btrfs send`/`recv` over ssh; DRBD/zfs if available) + **one active gateway** + failover/failback: `deploy/ha_remote.sh` on two nodes, `deploy/ha.sh` for a single-host drill | no new code |
| **F2 — horizontal** | *if one node's scale/throughput stops being enough* | substrate = **SeaweedFS/RustFS** (already validated with `celld diagnose`); `us-s3gw` only if it still adds the CAS | swap, not fork |

One rule spans all three: **one active writer per bucket/root** (the CAS is a process-wide lock);
F1 keeps one gateway active (the standby is promoted by hand/`ha_remote.sh`), F2 moves the lock into a
substrate that already enforces conditional writes.

**F1 is implemented and proven (T76 rehearsal, T87 real hardware).** `deploy/ha.sh` is the
single-host drill; **`deploy/ha_remote.sh` is the two-node production path** used on server-1 /
server-2: `replicate` takes a read-only btrfs snapshot of the live subvolume and streams it with
`btrfs send [-p parent] | ssh peer btrfs receive` (full 31.9 MB in 6.03 s, incremental in
1.6–2.7 s); `promote` fences the peer, turns the newest received snapshot into the live `data`
subvolume, starts the gateway and runs `celld diagnose` + `scripts/race.sh` as the promotion gate
(**failover 3.18 s, failback 4.80 s** fence→PASS); `promote` **refuses** while the peer or the local
gateway is still active — two gateways would each hold their own CAS lock and both could own a
cell. On these NixOS nodes DRBD/zfs were not usable (in-kernel drbd 8.4 has no userspace; the
nixpkgs DRBD 9 module is marked broken; ZFS does not evaluate), so `btrfs send/recv` is the
filesystem-level mechanism — see [`1us-infra/docs/fleet-storage-runbook.md`](../infra/docs/fleet-storage-runbook.md) §7.

## Known limits (honest)

- **Single process.** The CAS lock is what makes the conditional write atomic; it lives in one
  process's memory. **Never run two `us-s3gw` instances against the same root** — that reintroduces
  the race the lock exists to close (one store per root, always).
- **One store per root, N celld nodes per store.** The lock is per-process, not per-connection: a
  single store instance safely serves any number of concurrent celld nodes (`scripts/race.sh` is
  exactly that shape: 24 concurrent writers → 1 winner).
- **No TLS here.** Put Caddy (or any ingress) in front and point celld at that; the store speaks
  plain HTTP with one static key pair, no IAM, no per-tenant credentials.
- Everything else is `s3s-fs` as upstream ships it: filesystem storage, single static key pair.
  A shared backend (Postgres/etcd) is the answer if multi-process CAS is ever needed.
- **The secret key is passed as a flag**, so it is visible in `ps`/`systemctl status` to the service
  user and root. The system unit runs as a dedicated `us-s3gw` user; `hidepid=2` hides it from
  other users. Reading the key from a file instead is a code change, out of F0 scope.

## Still to do

- Multi-node / shared-backend CAS (single-process lock is the current contract).
- Run `s3s-test` / a subset of `ceph/s3-tests` in CI.

## Provenance and licence

Derived from **`s3s-fs` 0.14.1** (`github.com/s3s-project/s3s`), Apache-2.0, © its authors.
See `NOTICE`. The upstream licence text is kept in `LICENSE.s3s-fs`.
