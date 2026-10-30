// Copyright 2026 Deno Land Inc. Apache-2.0 license.

import { DurableObject, RpcTarget } from "cloudflare:workers";

// Models a stub from an RPC system that lives above celld (Cap'n Web). The caller's copy is
// released once the call completes, so a receiver that keeps it must hold a `dup()`: this is the
// rule the Workshop's chat subscription relies on.
const hooks = new Map();
let nextHook = 1;

class Callback extends RpcTarget {
  constructor(hook) {
    super();
    this.hook = hook;
  }

  dup() {
    this.hook.refs += 1;
    return new Callback(this.hook);
  }

  [Symbol.dispose]() {
    if (this.hook.refs > 0) this.hook.refs -= 1;
  }

  report(value) {
    if (this.hook.refs === 0) throw new Error("retained stub was already released");
    this.hook.reports.push(value);
    return value;
  }
}

class Lease extends RpcTarget {
  alive() {
    return "true";
  }
}

export class RetainedCallback extends DurableObject {
  constructor(ctx) {
    super(ctx);
    this.callback = null;
    this.outcome = null;
  }

  async retain(callback) {
    // What a Workspace DO does with its chat subscriber: keep the capability past the call.
    this.callback = callback.dup();
    await this.ctx.storage.setAlarm(Date.now() + 250);
    return "retained";
  }


  async persist(callback) {
    try {
      await this.ctx.storage.put("cb", callback);
      return "stored";
    } catch (error) {
      return "refused:" + String(error);
    }
  }

  async alarm() {
    if (this.callback === null) {
      this.outcome = "err:no retained callback";
      return;
    }
    try {
      await this.callback.report("alarm");
      this.outcome = "ok";
    } catch (error) {
      this.outcome = "err:" + String(error?.message ?? error);
    }
  }

  // Re-entrant: the receiver calls the callback while the call that delivered it is still being
  // served. This is what a subscription does (streamGeneration and the initial replay happen
  // inside subscribeToChat), and it is the case that a session-level repair depends on.
  async during(callback) {
    this.callback = callback.dup();
    return "during:" + await this.callback.report("during");
  }

  async result() {
    return this.outcome;
  }

  async lease() {
    return new Lease();
  }
}

let session = null;

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const cell = () => env.DO.get(env.DO.idFromName("retained"));
    if (url.pathname === "/start") {
      const hook = { refs: 1, reports: [] };
      hooks.set(nextHook, hook);
      const callback = new Callback(hook);
      nextHook += 1;
      await cell().retain(callback);
      // Cap'n Web disposes the callee's copy of a parameter stub when the call completes.
      callback[Symbol.dispose]();
      return new Response("started");
    }
    if (url.pathname === "/persist") {
      const hook = { refs: 1, reports: [] };
      hooks.set(nextHook, hook);
      const callback = new Callback(hook);
      nextHook += 1;
      const result = await cell().persist(callback);
      callback[Symbol.dispose]();
      return new Response(result);
  }

  if (url.pathname === "/during") {
      const hook = { refs: 1, reports: [] };
      hooks.set(nextHook, hook);
      const callback = new Callback(hook);
      nextHook += 1;
      const outcome = await cell().during(callback);
      callback[Symbol.dispose]();
      return new Response(String(outcome));
    }
    if (url.pathname === "/result") return new Response(String(await cell().result()));
    if (url.pathname === "/lease") {
      const lease = await cell().lease();
      return new Response(String(await lease.alive()));
    }
    if (url.pathname === "/socket") {
      if (request.headers.get("Upgrade")?.toLowerCase() !== "websocket") {
        return new Response("websocket upgrade required", { status: 426 });
      }
      const pair = new WebSocketPair();
      const [client, socket] = Object.values(pair);
      socket.accept();
      session = socket;
      // The production shape: the caller is a WebSocket message handler that is awaiting the
      // Durable Object call when the object calls back into this isolate.
      socket.addEventListener("message", (event) => {
        void (async () => {
          try {
            const hook = { refs: 1, reports: [] };
            hooks.set(nextHook, hook);
            const callback = new Callback(hook);
            nextHook += 1;
            const outcome = await cell().during(callback);
            callback[Symbol.dispose]();
            socket.send("result:" + outcome);
          } catch (error) {
            socket.send("error:" + String(error?.message ?? error));
          }
        })();
      });
      return new Response(null, { status: 101, webSocket: client });
    }
    return new Response("not found", { status: 404 });
  },
};
