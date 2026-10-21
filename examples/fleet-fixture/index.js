// Runtime 2-node fixture: one Durable Object cell, exercised over HTTP.
import { DurableObject, RpcTarget, restore } from "cloudflare:workers";

class DurableValue extends RpcTarget {
  async value() { return "durable-value"; }
}

class PingTarget extends RpcTarget {
  ping() { return "pong"; }
}

export class Cell extends DurableObject {
  async [restore](params) {
    if (params?.type === "value") return new DurableValue();
    throw new TypeError("unknown restore params");
  }
  async hello() { return "hello-from-cell"; }
  async stream() {
    const bytes = new TextEncoder().encode("cross-node-stream");
    let offset = 0;
    return new ReadableStream({
      type: "bytes",
      pull(controller) {
        if (offset < bytes.byteLength) controller.enqueue(bytes.slice(offset, offset += 4));
        else controller.close();
      },
    });
  }
  async waitAndSet(key, ms) {
    await new Promise((resolve) => setTimeout(resolve, ms));
    await this.ctx.storage.put(key, true);
    return "done";
  }
  async flag(key) { return (await this.ctx.storage.get(key)) === true; }
  async storeDurable() {
    const value = await this.ctx.restore({ type: "value" });
    await this.ctx.storage.put("durable", value);
    return "stored";
  }
  async durable() {
    const value = await this.ctx.storage.get("durable");
    return value ? await value.value() : "missing";
  }
  async callback(target) {
    try { return await target.ping(); }
    catch (error) { return `err:${error?.code ?? error?.name ?? error}`; }
  }
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const cell = env.CELL.get(env.CELL.idFromName("fleet"));
    if (url.pathname === "/hello") return new Response(await cell.hello());
    if (url.pathname === "/stream") {
      const stream = await cell.stream();
      const reader = stream.getReader({ mode: "byob" });
      let text = "";
      for (;;) {
        const { value, done } = await reader.read(new Uint8Array(7));
        if (done) break;
        text += new TextDecoder().decode(value);
      }
      return new Response(text);
    }
    if (url.pathname === "/slow") return new Response(await cell.waitAndSet("slow", 1500));
    if (url.pathname === "/flag") return new Response(String(await cell.flag(url.searchParams.get("key") ?? "")));
    if (url.pathname === "/store") return new Response(await cell.storeDurable());
    if (url.pathname === "/durable") return new Response(await cell.durable());
    if (url.pathname === "/stateless") return new Response(await cell.callback(new PingTarget()));
    return new Response("ok");
  },
};
