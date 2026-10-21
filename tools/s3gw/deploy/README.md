# us-s3gw as a system service (systemd)

`us-s3gw` is a **single-process** store: its atomic CAS is a process-wide lock, so the
deployment contract is **one writer per bucket/root** (see the main README, "Known limits").
These units run it as a durable service: start at boot, restart on failure, data on a
dedicated filesystem, credentials outside the repo.

## Files

| file | install to | role |
|---|---|---|
| `us-s3gw.service` | `/etc/systemd/system/us-s3gw.service` | system service (production host) |
| `us-s3gw.user.service` | `~/.config/systemd/user/us-s3gw.service` | user service (developer host / no root) |
| `us-s3gw.env.example` | `/etc/us-s3gw/us-s3gw.env` or `~/.config/us-s3gw/us-s3gw.env` | config + credentials (mode 0600, **never in git**) |
| `us-s3gw-backup.service` + `.timer` | same dirs | periodic mirror + snapshot (see `deploy/backup.sh`) |

The binary takes **flags**, so the config is an `EnvironmentFile` and the unit expands
`${S3GW_*}` in `ExecStart` (that keeps secrets out of the unit and out of the repo).

## Install (production host, root)

```sh
install -m 0755 us-s3gw /usr/local/bin/us-s3gw
install -d -m 0750 /etc/us-s3gw
install -m 0600 us-s3gw.env.example /etc/us-s3gw/us-s3gw.env   # then edit it
install -m 0644 us-s3gw.service /etc/systemd/system/us-s3gw.service
install -d -m 0750 -o us-s3gw -g us-s3gw /var/lib/us-s3gw       # dedicated FS (RAID1), see runbook
systemctl daemon-reload
systemctl enable --now us-s3gw
systemctl status us-s3gw --no-pager
```

`/var/lib/us-s3gw` must be its **own filesystem** (F0: RAID1; see the runbook in
`1us-infra/docs/fleet-storage-runbook.md`). If `S3GW_ROOT` points somewhere else, update
`ReadWritePaths=` in the unit to match.

## Install (developer host, no root)

```sh
install -m 0755 target/release/us-s3gw ~/.local/bin/us-s3gw
install -d -m 0700 ~/.config/us-s3gw
install -m 0600 us-s3gw.env.example ~/.config/us-s3gw/us-s3gw.env   # then edit it
install -m 0644 us-s3gw.user.service ~/.config/systemd/user/us-s3gw.service
systemctl --user daemon-reload
systemctl --user enable --now us-s3gw
systemctl --user status us-s3gw --no-pager
```

Boot start for a user service requires lingering:

```sh
loginctl enable-linger "$USER"     # once; check: loginctl show-user "$USER" | grep Linger
```

## Config (`us-s3gw.env`)

```
S3GW_HOST=127.0.0.1
S3GW_PORT=9020
S3GW_ROOT=/var/lib/us-s3gw        # user unit default: %h/us-s3gw-data
S3GW_ACCESS_KEY=change-me         # the celld `AWS_ACCESS_KEY_ID`
S3GW_SECRET_KEY=change-me         # the celld `AWS_SECRET_ACCESS_KEY`
```

Keep the file `0600` and outside the repo; `.env`/`*.env` are gitignored in `s3gw/`.

## Operate

```sh
systemctl (--user) start|stop|restart|status us-s3gw
journalctl (--user) -u us-s3gw -f            # logs
```

Backups (mirror + snapshot) are described in `deploy/backup.sh`; enable the timer with
`systemctl (--user) enable --now us-s3gw-backup.timer`. Restoration and failover live in
the runbook (`1us-infra/docs/fleet-storage-runbook.md`).

## Backups (`deploy/backup.sh`) and restore (`deploy/restore.sh`)

The script reads the same `S3GW_*` env as the service and does two things:

- **mirror** — `rsync -a --delete "$S3GW_ROOT/" "$S3GW_MIRROR/"`: the RAID1 substitute when the
  host has no second disk (see the runbook for the real mdadm/DRBD/ZFS steps).
- **snapshot** — `rsync --link-dest`, i.e. a point-in-time tree under `$S3GW_SNAPSHOTS/<UTC>`
  that shares unchanged inodes with its predecessor (cheap, like a ZFS snapshot). Each snapshot
  carries a `MANIFEST`. `$S3GW_RETAIN` (default 7) newest are kept.

`S3GW_QUIESCE=1` stops the service for a cold, always-consistent copy (short downtime);
`S3GW_QUIESCE=0` is an online copy — safe per file (objects land via tmp+rename) but a copy taken
mid-flight can pair data with a newer metadata sidecar, so use quiesce for anything you will
restore. Runs are serialized with `flock`.

```sh
# manual, cold, to the paths in us-s3gw.env:
S3GW_QUIESCE=1 S3GW_SYSTEMD_SCOPE=user ~/.local/bin/us-s3gw-backup
# restore the newest snapshot into a fresh dir and bring a store back on it:
~/.local/bin/us-s3gw-restore --snapshot ~/us-s3gw-snapshots/20261003T... --target ~/us-s3gw-restored
```

For a system install, replace `%h/.local/bin/…` with `/usr/local/bin/…` in the backup units
(`ExecStart=`) and drop `Environment=S3GW_SYSTEMD_SCOPE=user` (the default is `system`).

## HA (F1): `deploy/ha.sh`

`us-s3gw` is a one-writer store (the CAS is a process lock), so HA is **one active gateway + a
replicated disk + a promotion procedure** — never two gateways. `deploy/ha.sh` encodes that
procedure so the drill and the runbook cannot drift:

```sh
export S3GW_ACCESS_KEY=… S3GW_SECRET_KEY=…        # or: . ~/.config/us-s3gw/us-s3gw.env
export S3GW_BIN=~/.local/bin/us-s3gw

deploy/ha.sh init          # primary (+ btrfs subvolume when possible) and replica dir
deploy/ha.sh start         # gateway for the active side (systemd-run --user)
deploy/ha.sh replicate     # btrfs COW snapshot of the primary -> replica (+ hash verify)
deploy/ha.sh status        # active_side, epoch, units, replica age, file/byte counts
deploy/ha.sh failover      # fence primary -> promote replica -> start -> gates (diagnose+race)
deploy/ha.sh failback      # stop promoted -> sync replica -> primary -> start -> gates
deploy/ha.sh stop
```

State lives in `$S3GW_HA_DIR` (default `~/us-s3gw-f1`): `state/active_side`, `state/epoch`,
`state/REPLICA_STATE`, `snapshots/`. **Nothing but store objects is ever written into a root.**

The guard that matters: `failover` **refuses** while the primary gateway is still active (that is
the two-writers/two-owners failure) and refuses a stale replica (`S3GW_MAX_STALENESS`, default
900 s) unless `--force`. Promotion runs `scripts/accept.sh` and `scripts/race.sh` against the new
gateway and rolls back the promotion if either fails.

Replication here is filesystem-level: a **btrfs read-only COW snapshot** of the live root (instant,
consistent) synced to the standby. On real hardware it is **DRBD** (block) or **`btrfs send` /
`zfs send`** to the second host; this repo's reference host has one disk and no root, so the drill
uses the same procedure with a second directory and documents the send/DRBD steps (runbook §7).

## HA (F1) on two real nodes: `deploy/ha_remote.sh`

`deploy/ha.sh` above is the **single-host drill** (rsync into a replica directory). For the real
two-node deployment (T87: server-1/server-2, dedicated btrfs disk each) the tool is
`deploy/ha_remote.sh`; it runs **on a node** as root and reads the instance env file
(`/etc/us-s3gw-ha/us-s3gw-ha.env`: `S3GW_{PORT,ROOT,BUCKET,ACCESS_KEY,SECRET_KEY,PEER_HOST}`).

```sh
ha_remote.sh install      # copy the unit into /run/systemd/system (NixOS /etc is static) + reload
ha_remote.sh status       # unit, snapshots sent, received, newest, usage
ha_remote.sh replicate    # on the ACTIVE node: btrfs snapshot -> btrfs send | ssh peer btrfs receive
ha_remote.sh promote      # on the STANDBY: fence check -> promote newest received -> start -> gates
ha_remote.sh demote       # stop the local gateway (fence / standby)
ha_remote.sh gates        # celld diagnose + s3probe race against the local gateway
```

Replication is filesystem-level (`btrfs send [-p parent]`, incremental after the first full send).
`promote` **refuses while the local or the peer gateway is active** (`two writers would each hold
their own CAS lock (two cell owners)`), swaps `data` for a writable snapshot of the newest received
one (keeping the received snapshot as the next incremental parent), starts the unit and requires
`celld diagnose` = `ok bucket conditional write` plus `race.sh` = 1 winner per round before calling
the promotion good. Measured on server-1/server-2: full send 6.03 s, incremental 1.6–2.7 s,
failover 3.18 s and failback 4.80 s (fence → gates PASS). The unit body lives in
`deploy/systemd/us-s3gw-ha.service`; the runbook (`1us-infra/docs/fleet-storage-runbook.md` §7) has
the full install (dedicated disks, subvolume mount units, NixOS declarative equivalent, firewall).
