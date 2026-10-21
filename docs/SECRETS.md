# Fleet secrets

celld deployments can bind secrets without ever putting their values in the
config, the bundle, the manifest, or the node log.

## Threat model

Protected:

- the Fleet bucket (objects, manifests, deployment pointers, backups) and its
  readers: a secret value is only present there as AES-256-GCM ciphertext;
- the deploy CLI's stdout/`--json`, the node log, tail logs and error messages:
  decrypted values are registered with the redactor and replaced with
  `[redacted]`;
- the config and the bundled Worker: they name secrets, never carry values.

Not protected:

- a compromised node process: `CELLD_SECRET_KEY` is in its environment and it
  must decrypt to serve the binding; anyone who can read the node env can read
  every secret of every deployment that node serves;
- the deploy host: the operator's shell history/files and the deploy process
  memory are outside the boundary; values come from files, not argv, but the
  host still sees them;
- Worker code: a Worker can `console.log(env.SECRET)`. The redactor rewrites
  the known values in runtime logs, but application code can encode, hash or
  send them elsewhere; this is a data-handling control, not a sandbox;
- the fleet key is shared by all nodes and deployments. There is no per-secret
  or per-script key, and no access control between workloads on one node.

## Format and key

- Config: `secrets` is an object of names to binding kinds:
  `{"API_TOKEN": "text", "TLS_KEY": "key"}`. Values never appear in the config.
  `text` becomes a string binding; `key` becomes an `ArrayBuffer` binding.
- Deploy: `celld deploy --secret API_TOKEN=@token.txt --secret-file TLS_KEY=key.pem`.
  A literal `NAME=value` is refused; values are read from files (or `@-` for
  one stdin secret). Missing, extra or duplicate values are deploy errors.
- Key: `CELLD_SECRET_KEY`, 32 bytes as 64 hex characters, in the deploy process
  and in every node process. It is never written to the bucket. A node that
  cannot parse it refuses a deployment that carries secrets and keeps serving
  its previous generation.
- At rest: `<version-prefix>/secrets.json` is a JSON envelope with a format
  version, the script name, and one entry per secret:
  `{"name", "kind", "nonce", "ciphertext"}` (base64). Each value is encrypted
  independently with AES-256-GCM; the 96-bit nonce is random and stored beside
  the ciphertext. The AEAD additional data binds the format version, script
  name, secret name and kind, so an entry cannot be moved to another script,
  name or kind. The envelope's SHA-256 is in the manifest and is part of the
  deployment version, so rotating a value always produces a new deployment.
- Bootstrap: the node reads the envelope, verifies its hash, decrypts each
  entry once while loading the generation, injects the `secret_text`/
  `secret_key` binding into the Worker environment, and registers text values
  with the redactor. Isolates share one `Arc<WorkerConfig>`; there is no
  per-request decrypt.

## Rotation and rolling deploys

- Rotating a value deploys a new version (the ciphertext hash changes). Nodes
  adopt it like any other deployment. A node that has not adopted yet keeps
  the previous generation, and therefore the previous value; this is the
  rolling behavior described in `docs/rolling-deploy.md`.
- A deployment with secrets requires the `secrets_v1` deployment feature.
  Nodes that implement the required-features check refuse that version before
  loading code and keep serving their current generation. A node that predates
  that check (or any pre-C build without it) cannot be relied on to refuse a
  manifest it does not understand; it can serve the deployment with the
  `secret_text`/`secret_key` bindings missing. There is no way to make an old
  binary enforce a feature it does not know, so **upgrade every node before
  the first secrets deploy**. The E2E's pre-C probe is informational for that
  reason.
- Rotating `CELLD_SECRET_KEY` (the fleet key) cannot recover old values: there
  is no plaintext in the bucket. Redeploy every secret-bearing Worker with its
  values under the new key, then roll the nodes. Drain a node before changing
  its key; until it is drained and restarted it can only decrypt generations
  made with the key it has.

## Tests

`secrets.rs` covers key parsing, text/key round trips, wrong key, tampering,
AAD/version rejection, missing key, envelope non-disclosure and redaction.
`examples/secrets-fixture` + `scripts/e2e-secrets.sh` deploy a real Worker with
text and key secrets, prove the bucket holds no plaintext, fetch the bindings
from a node, and check the missing/wrong-key failures.
