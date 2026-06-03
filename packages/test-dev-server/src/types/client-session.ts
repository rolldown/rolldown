import type { WebSocket } from 'ws';

export class ClientSession {
  id: string;
  ws: WebSocket;
  registeredModules = new Set<string>();

  constructor(ws: WebSocket, id: string) {
    this.id = id;
    this.ws = ws;
  }
}
