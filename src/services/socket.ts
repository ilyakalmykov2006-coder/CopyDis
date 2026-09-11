import type { ClientEvent, ServerEvent } from "../types/protocol";
export class RoomSocket {
  private ws?: WebSocket; private retry = 0; private stopped = false;
  constructor(private readonly url: string, private readonly onEvent: (e: ServerEvent) => void, private readonly onState: (s: "connected" | "reconnecting" | "offline") => void) {}
  connect() { this.stopped = false; this.open(); }
  private open() { this.onState(this.retry ? "reconnecting" : "offline"); this.ws = new WebSocket(this.url); this.ws.onopen = () => { this.retry = 0; this.onState("connected"); }; this.ws.onmessage = (e) => this.onEvent(JSON.parse(e.data) as ServerEvent); this.ws.onclose = () => { if (!this.stopped) { this.retry++; this.onState("reconnecting"); setTimeout(() => this.open(), Math.min(1000 * 2 ** this.retry, 15000)); } else this.onState("offline"); }; this.ws.onerror = () => this.ws?.close(); }
  send(event: ClientEvent) { if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(event)); }
  close() { this.stopped = true; this.ws?.close(); }
}
