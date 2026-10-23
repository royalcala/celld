// A minimal reproduction of the `executeCode` shape: a Dynamic Worker whose
// RPC entrypoint logs, with a Tail Worker loopback. The caller waits for the
// tail event and returns it, so a missing tail delivery is a timeout here.

import { DurableObject, WorkerEntrypoint } from "cloudflare:workers";

const AGENT = `
import { WorkerEntrypoint } from "cloudflare:workers";

export default class Agent extends WorkerEntrypoint {
  async verify() {
    return "ok";
  }

  async run() {
    console.log("hello from executeCode", 42);
    console.log({ nested: true });
    return "run-result";
  }
}
`;

export class Collector extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    this.ctx.storage.sql.exec(
      "CREATE TABLE IF NOT EXISTS events (body TEXT NOT NULL)");
  }

  async fetch(request) {
    const url = new URL(request.url);
    if (request.method === "POST" && url.pathname === "/events") {
      const events = await request.json();
      for (const event of events) {
        this.ctx.storage.sql.exec(
          "INSERT INTO events (body) VALUES (?)", JSON.stringify(event));
      }
      return Response.json({ ok: true });
    }
    if (url.pathname === "/clear") {
      this.ctx.storage.sql.exec("DELETE FROM events");
      return Response.json({ ok: true });
    }
    if (url.pathname === "/snapshot") {
      const rows = [
        ...this.ctx.storage.sql.exec("SELECT body FROM events ORDER BY rowid"),
      ];
      return Response.json(rows.map((row) => JSON.parse(row.body)));
    }
    return new Response("not found", { status: 404 });
  }
}

export class TailLoopback extends WorkerEntrypoint {
  async tail(events) {
    const collector = this.env.COLLECTOR.get(this.env.COLLECTOR.idFromName("trace"));
    await collector.fetch("https://collector/events", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(events),
    });
  }
}

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    if (url.pathname === "/health") return new Response("ok");
    if (url.pathname !== "/run") return new Response("not found", { status: 404 });

    const collector = env.COLLECTOR.get(env.COLLECTOR.idFromName("trace"));
    await collector.fetch("https://collector/clear", { method: "POST" });

    const worker = env.LOADER.load({
      compatibilityDate: "2026-02-01",
      mainModule: "agent.js",
      modules: { "agent.js": AGENT },
      tails: [ctx.exports.TailLoopback({ props: { executionId: "fixture" } })],
      globalOutbound: null,
    });
    const entrypoint = worker.getEntrypoint("default");

    // The product's order: verify(), then run(). Both are RPC calls.
    await entrypoint.verify();
    const result = await entrypoint.run();

    const deadline = Date.now() + 5000;
    let events = [];
    while (Date.now() < deadline) {
      events = await (await collector.fetch("https://collector/snapshot")).json();
      if (events.some((event) => event.event?.rpcMethod === "run")) break;
      await scheduler.wait(25);
    }
    const run = events.find((event) => event.event?.rpcMethod === "run");
    return Response.json({
      timedOut: run === undefined,
      result,
      events,
      logs: run?.logs ?? [],
    });
  },
};
