// Copyright 2026 Deno Land Inc. Apache-2.0 license.

// Models a Workspace Durable Object that keeps a subscription capability which
// lives in a browser session: the stub cannot be stored, the token can, and a
// delivery after the object forgot every handle must go through the stored one.

import {
  DurableObject,
  RpcTarget,
  capabilityToken,
  restore,
  restoreCapability,
} from "cloudflare:workers";

// The session capability. It is a transient handle: it belongs to the isolate
// that created it, which is exactly why its stub has no durable identity.
class Session extends RpcTarget {
  report(text) {
    return "session:" + text;
  }
}

export class Subscriber extends DurableObject {
  constructor(ctx) {
    super(ctx);
    // Nothing is kept in memory: every later delivery has to come from storage,
    // the way a restored object has nothing but its rows.
    this.callback = null;
  }

  async [restore](_params) {
    return new Session();
  }

  // What a Workspace DO does with its chat subscriber: keep the capability past
  // the call that delivered it.
  async arm(callback) {
    const token = capabilityToken(callback);
    await this.ctx.storage.put("cb", token);
    return "armed:" + typeof token + ":" + token.length;
  }

  // A token is a string, so it round-trips through Durable Object storage with
  // no stub envelope at all.
  async stored() {
    const value = await this.ctx.storage.get("cb");
    return typeof value + ":" + String(value).length;
  }

  // The delivery a restored object makes: no live handle, only the row.
  async deliver() {
    const token = await this.ctx.storage.get("cb");
    if (typeof token !== "string") return "no-token";
    const callback = restoreCapability(token);
    try {
      return "delivered:" + (await callback.report("alarm"));
    } catch (error) {
      return "err:" + (error?.code ?? error?.message ?? error);
    }
  }

  // The refusal the storage rule states, asserted as a contract.
  async persistStub(callback) {
    try {
      await this.ctx.storage.put("stub", callback);
      return "stored";
    } catch (error) {
      return "refused:" + String(error);
    }
  }

  // A forged or truncated token never revives.
  async forged() {
    try {
      return "revived:" + typeof restoreCapability("not-a-token");
    } catch (error) {
      return "err:" + String(error?.code ?? "none") + ":" +
        String(error?.message ?? error);
    }
  }

  // Neither a plain object nor a durable handle is a transient capability.
  async notACapability(value) {
    try {
      return "token:" + capabilityToken(value);
    } catch (error) {
      return "err:" + String(error?.message ?? error);
    }
  }

  async durable() {
    const stub = this.ctx.restore({ type: "value" });
    try {
      return "token:" + capabilityToken(stub);
    } catch (error) {
      return "err:" + String(error?.message ?? error);
    }
  }
}

let session = null;

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const cell = () => env.DO.get(env.DO.idFromName("subscriber"));
    if (url.pathname === "/session") {
      if (request.headers.get("Upgrade")?.toLowerCase() !== "websocket") {
        return new Response("websocket upgrade required", { status: 426 });
      }
      const pair = new WebSocketPair();
      const [client, socket] = Object.values(pair);
      socket.accept();
      session = socket;
      // The production shape: a session's message handler is awaiting the
      // Durable Object call when the object takes (or uses) the capability.
      socket.addEventListener("message", (event) => {
        void (async () => {
          try {
            if (event.data === "arm") {
              socket.send(await cell().arm(new Session()));
              return;
            }
            if (event.data === "deliver") {
              socket.send(await cell().deliver());
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
    if (url.pathname === "/stored") return new Response(await cell().stored());
    if (url.pathname === "/persist") {
      return new Response(await cell().persistStub(new Session()));
    }
    if (url.pathname === "/forged") return new Response(await cell().forged());
    if (url.pathname === "/not-a-capability") {
      return new Response(await cell().notACapability({ plain: "object" }));
    }
    if (url.pathname === "/durable") return new Response(await cell().durable());
    return new Response("not found", { status: 404 });
  },
};
