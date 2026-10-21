// Fleet fixture for encrypted secrets + observability. The deploy supplies the
// secret values outside the config; this worker proves the bootstrap injected
// them and exercises request ids, spans, RPC and a cross-isolate stream.
import { DurableObject, tracing } from "cloudflare:workers";

export class Counter extends DurableObject {
  async ping() {
    return "pong";
  }

  async bytes() {
    const payload = new TextEncoder().encode("stream-data");
    let offset = 0;
    return new ReadableStream({
      type: "bytes",
      pull(controller) {
        if (offset >= payload.byteLength) {
          controller.close();
          return;
        }
        const end = Math.min(offset + 4, payload.byteLength);
        controller.enqueue(payload.slice(offset, end));
        offset = end;
      },
    });
  }
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname === "/token") {
      return new Response(env.API_TOKEN);
    }
    if (url.pathname === "/key") {
      if (!(env.TLS_KEY instanceof ArrayBuffer)) {
        return new Response("not-array-buffer", { status: 500 });
      }
      const bytes = new Uint8Array(env.TLS_KEY);
      return new Response(
        `${bytes.byteLength}:${Array.from(bytes.slice(0, 4)).join(",")}`,
      );
    }
    if (url.pathname === "/rpc") {
      const counter = env.COUNTER.get(env.COUNTER.idFromName("obs"));
      return new Response(await counter.ping());
    }
    if (url.pathname === "/stream") {
      const counter = env.COUNTER.get(env.COUNTER.idFromName("obs"));
      const stream = await counter.bytes();
      const reader = stream.getReader({ mode: "byob" });
      let text = "";
      for (;;) {
        const { value, done } = await reader.read(new Uint8Array(7));
        if (done) break;
        text += new TextDecoder().decode(value);
      }
      return new Response(text);
    }
    if (url.pathname === "/trace") {
      return tracing.enterSpan(
        "e2e-span",
        (span) => {
          span?.setAttribute("route", "/trace");
          span?.setAttribute("token", env.API_TOKEN);
          const active = tracing.getActiveSpan();
          return new Response(
            JSON.stringify({
              traced: tracing.isTraced(),
              name: active?.name ?? null,
              requestId: request.headers.get("x-celld-request-id"),
            }),
            { headers: { "content-type": "application/json" } },
          );
        },
        { attributes: { fixture: "obs" } },
      );
    }
    return new Response("ok");
  },
};
