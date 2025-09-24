import { WebSocket } from 'ws';

let id = 0;

export class ClientSession {
  id = `${id++}`;
  ws: WebSocket;

  constructor(ws: WebSocket) {
    this.ws = ws;
  }
}
