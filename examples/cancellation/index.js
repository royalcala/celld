// Runtime evidence for caller-to-callee cancellation.
//
// The caller is a stateless default-export fetch; the callee is a Durable
// Object method. Each endpoint surfaces one path:
//   /slow     caller disconnects while the callee awaits a timer
//   /abort    caller calls ctx.abort() while the call is pending
//   /detach   caller drops the call without awaiting it (event ends)
//   /busy     a callee turn that never yields (cancellation at a turn boundary)
import { DurableObject } from "cloudflare:workers";

export class TestDO extends DurableObject {
  async waitAndSet(key, ms) {
    await new Promise((resolve) => setTimeout(resolve, ms));
    await this.ctx.storage.put(key, true);
    return `done:${key}`;
  }

  async busyAndSet(key, spins) {
    let sink = 0;
    for (let i = 0; i < spins; i += 1) sink = (sink + i) | 0;
    if (sink === 42) throw new Error("unreachable");
    await this.ctx.storage.put(key, true);
    return `done:${key}`;
  }

  async flag(key) {
    return (await this.ctx.storage.get(key)) === true;
  }

  async clear() {
    await this.ctx.storage.deleteAll();
    return "cleared";
  }
}

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    const stub = env.TEST_DO.get(env.TEST_DO.idFromName("cancellation"));
    if (url.pathname === "/slow") {
      return new Response(await stub.waitAndSet("slow", 1500));
    }
    if (url.pathname === "/busy") {
      return new Response(await stub.busyAndSet("busy", 700_000_000));
    }
    if (url.pathname === "/flag") {
      return new Response(String(await stub.flag(url.searchParams.get("key") ?? "")));
    }
    if (url.pathname === "/abort") {
      const pending = stub.waitAndSet("abort", 1500)
        .catch((error) => `err:${error?.name ?? error}`);
      ctx.abort(new Error("test abort"));
      return new Response(await pending);
    }
    if (url.pathname === "/detach") {
      stub.waitAndSet("detach", 1500).catch(() => {});
      return new Response("detached");
    }
    if (url.pathname === "/clear") {
      return new Response(await stub.clear());
    }
    return new Response("ok");
  },
};
