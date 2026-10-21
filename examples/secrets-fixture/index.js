// Fleet secrets fixture: one `secret_text` and one `secret_key` binding.
// The deploy supplies the values outside the config; this worker only proves
// that the bootstrap injected them into `env`.
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
    return new Response("ok");
  },
};
