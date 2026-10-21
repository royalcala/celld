// T42 live-size probe app: one DO that absorbs sequential chunks so the node
// captures LTX segments and (past the compaction trigger) publishes L1
// objects into the fleet bucket. Used by scripts/live-sizes.sh to measure the
// object sizes celld actually stores against us-s3gw.
//
//   POST /?k=<n>  body=chunk  -> storage.put("k<n>", chunk)  (distinct keys
//                                grow the cell DB; re-writing keys dirties
//                                pages without growing it)
//   GET  /status              -> { puts, bytes }
export class Bulk {
  constructor(state) {
    this.state = state;
  }
  async fetch(request) {
    const url = new URL(request.url);
    if (url.pathname === "/status") {
      const n = (await this.state.storage.get("n")) ?? 0;
      const bytes = (await this.state.storage.get("bytes")) ?? 0;
      return Response.json({ puts: n, bytes });
    }
    const chunk = new Uint8Array(await request.arrayBuffer());
    if (chunk.byteLength === 0) {
      return Response.json({ error: "empty chunk" }, { status: 400 });
    }
    const key = url.searchParams.get("k") ?? "blob";
    const n = (await this.state.storage.get("n")) ?? 0;
    const bytes = (await this.state.storage.get("bytes")) ?? 0;
    await this.state.storage.put(`k${key}`, chunk);
    await this.state.storage.put("n", n + 1);
    await this.state.storage.put("bytes", bytes + chunk.byteLength);
    return Response.json({ puts: n + 1, bytes: chunk.byteLength });
  }
}

export default {
  async fetch(request, env) {
    const id = env.BULK.idFromName("probe");
    return env.BULK.get(id).fetch(request);
  },
};
