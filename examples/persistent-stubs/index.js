// Runtime evidence for durable `ctx.restore`:
//   - a facet mints its own persistent stub through the parent chain;
//   - the stub is stored in the facet's Durable Object storage;
//   - after a process restart the root replays `[restore]`, reopens the facet,
//     and the facet replays its own `[restore]`.
import { DurableObject, RpcTarget, restore } from "cloudflare:workers";

class Greeter extends RpcTarget {
  constructor(greeting) {
    super();
    this.greeting = greeting;
  }
  greet(name) {
    return `${this.greeting}, ${name}!`;
  }
}

export class Facet extends DurableObject {
  [restore](params) {
    if (params?.type === "greeter") return new Greeter(params.greeting);
    throw new Error("unknown restore params");
  }

  async make() {
    const greeter = await this.ctx.restore({ type: "greeter", greeting: "hello" });
    return await greeter.greet("world");
  }

  async store() {
    const greeter = await this.ctx.restore({ type: "greeter", greeting: "stored" });
    await this.ctx.storage.put("cb", greeter);
    return await greeter.greet("one");
  }

  async stored() {
    const greeter = await this.ctx.storage.get("cb");
    if (!greeter) return "missing";
    return await greeter.greet("two");
  }

  async dispose() {
    const greeter = await this.ctx.restore({ type: "greeter", greeting: "temp" });
    const answer = await greeter.greet("x");
    greeter[Symbol.dispose]();
    return answer;
  }
}

export class Root extends DurableObject {
  // The parent half of the chain. A facet has no published route, so its
  // persistent stub is owned by this root and replayed through this method.
  [restore](params) {
    if (params?.type === "facet") {
      return this.ctx.facets.get(params.name, () => ({
        class: this.ctx.exports.Facet,
        id: params.name,
      }));
    }
    throw new Error("unknown restore params");
  }

  async facetCall(name, method) {
    const facet = await this.ctx.restore({ type: "facet", name });
    return await facet[method]();
  }

  async storeAndDrop(name) {
    const facet = await this.ctx.restore({ type: "facet", name });
    const answer = await facet.store();
    // The stored child retains the parent row; dropping this handle cannot
    // break the token the facet persisted.
    facet[Symbol.dispose]();
    return answer;
  }
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const root = env.ROOT.get(env.ROOT.idFromName("persistent"));
    if (url.pathname === "/make") return new Response(await root.facetCall("f", "make"));
    if (url.pathname === "/store") return new Response(await root.storeAndDrop("f"));
    if (url.pathname === "/stored") return new Response(await root.facetCall("f", "stored"));
    if (url.pathname === "/dispose") return new Response(await root.facetCall("f", "dispose"));
    return new Response("ok");
  },
};
