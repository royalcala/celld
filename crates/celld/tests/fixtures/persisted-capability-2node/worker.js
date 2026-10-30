// Copyright 2026 Deno Land Inc. Apache-2.0 license.

// The two-node shape: the *session* (this default export, a stateless Worker)
// runs on node A, the *Durable Object* that keeps its subscription runs on node
// B. The object stores the capability's token, B restarts, and the alarm it
// armed restores the token and delivers to the session on A. Nothing on the
// client side repairs the subscription.

import {
  DurableObject,
  RpcTarget,
  capabilityToken,
  restoreCapability,
} from "cloudflare:workers";

// The alarm is far enough out that killing node B a few seconds after `arm`
// cannot race it, and close enough that the test does not wait long.
const ARM_MS = 20_000;

// The session capability: it lives in the stateless isolate and pushes frames on
// the socket that created it, which is how the test sees the delivery.
class Session extends RpcTarget {
  constructor(socket) {
    super();
    this.socket = socket;
  }

  report(text) {
    this.socket.send("delivered:" + text);
    return text;
  }
}

export class Subscriber extends DurableObject {
  // A Workspace object keeping its chat subscriber past the call that delivered
  // it: the stub cannot be stored, the token can.
  async arm(callback) {
    const token = capabilityToken(callback);
    await this.ctx.storage.put("cb", token);
    await this.ctx.storage.setAlarm(Date.now() + ARM_MS);
    return "armed";
  }

  async rearm(ms) {
    await this.ctx.storage.setAlarm(Date.now() + ms);
    return "rearmed";
  }

  // The restore: nothing but the row is left, so the handle comes back from the
  // token and the delivery crosses to node A.
  async alarm() {
    const token = await this.ctx.storage.get("cb");
    if (typeof token !== "string") {
      await this.ctx.storage.put("outcome", "no-token");
      return;
    }
    let outcome;
    try {
      const callback = restoreCapability(token);
      outcome = "delivered:" + await callback.report("alarm");
    } catch (error) {
      outcome = "err:" + (error?.code ?? error?.message ?? String(error));
    }
    await this.ctx.storage.put("outcome", outcome);
  }

  async outcome() {
    return (await this.ctx.storage.get("outcome")) ?? "none";
  }
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const cell = () => env.DO.get(env.DO.idFromName("subscriber"));
    // Never touches the object: a readiness probe must not claim the cell.
    if (url.pathname === "/ping") return new Response("pong");
    // Claims the cell for whichever node serves this.
    if (url.pathname === "/claim") return new Response(await cell().outcome());
    if (url.pathname === "/outcome") return new Response(await cell().outcome());
    if (url.pathname === "/session") {
      if (request.headers.get("Upgrade")?.toLowerCase() !== "websocket") {
        return new Response("websocket upgrade required", { status: 426 });
      }
      const pair = new WebSocketPair();
      const [client, socket] = Object.values(pair);
      socket.accept();
      socket.addEventListener("message", (event) => {
        void (async () => {
          try {
            if (event.data === "arm") {
              socket.send(await cell().arm(new Session(socket)));
              return;
            }
            if (event.data === "rearm") {
              socket.send(await cell().rearm(8000));
              return;
            }
            socket.send("unknown");
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
