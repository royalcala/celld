# Caller-to-callee cancellation

Runtime evidence for mid-flight cancellation of a cell RPC. A caller that
disconnects, aborts its context, or drops the call publishes the request's
abort; the cell drive loop samples it between turns, fails the event, and
releases its operations. The callee's committed turn is never rolled back.

## Run

```sh
celld dev examples/cancellation/wrangler.jsonc --port 18103 --no-watch --clean
B=http://127.0.0.1:18103
```

| Case | Command | Expected flag |
| --- | --- | --- |
| Completes normally | `curl $B/slow` | `true` |
| Caller disconnects while the callee awaits | `curl --max-time 0.25 $B/slow`, wait 2s, `curl "$B/flag?key=slow"` | `false` |
| Caller aborts its context | `curl $B/abort` (`err:Error`), wait 2s, `curl "$B/flag?key=abort"` | `false` |
| Caller drops the call (no await) | `curl $B/detach` (`detached`), wait 2s, `curl "$B/flag?key=detach"` | `false` |
| Non-yielding callee turn | `curl --max-time 0.25 $B/busy`, wait 1.5s, `curl "$B/flag?key=busy"` | `true` |

The last row is the documented limit: cancellation is observed at a turn
boundary. A callee that never yields cannot be interrupted inside its current
turn; the turn finishes, its committed write stays, and the runtime stops
waiting for the call instead of wedging the cell. `curl $B/` must answer `ok`
after every case.

`busyAndSet` is calibrated to take roughly half a second. `Date.now()` does
not advance within a turn, so the fixture spins a fixed count instead of
measuring time.
