export class AlarmSocket {
  constructor(state) {
    this.state = state;
    this.server = null;
  }

  async fetch(request) {
    if (request.headers.get("Upgrade")?.toLowerCase() !== "websocket") {
      return new Response("websocket upgrade required", { status: 426 });
    }
    const pair = new WebSocketPair();
    const [client, server] = Object.values(pair);
    this.server = server;
    server.accept();
    await this.state.storage.setAlarm(Date.now() + 250);
    return new Response(null, { status: 101, webSocket: client });
  }

  async alarm() {
    if (this.server) {
      this.server.send("desde-alarm");
    }
  }
}

export default {
  async fetch(request, env) {
    return env.DO.get(env.DO.idFromName("alarm")).fetch(request);
  },
};
