import type { ClientSession } from './types/client-session.js';

/**
 * Registry of connected HMR clients, mirroring the `Clients` class in Vite
 * full-bundle mode (`fullBundleEnvironment.ts`). A "client" here is a
 * {@link ClientSession} (its own id + websocket) rather than a Vite hot-channel
 * client, but the surface (`setupIfNeeded` / `get` / `getAll` / `delete`) is the
 * same.
 */
export class Clients {
  #byId = new Map<string, ClientSession>();

  get size(): number {
    return this.#byId.size;
  }

  /** Register a client, replacing a stale session that reconnected with the same id. */
  setupIfNeeded(client: ClientSession): void {
    const existing = this.#byId.get(client.id);
    if (existing && existing.ws !== client.ws) {
      console.warn(`Client ${client.id} reconnecting, replacing existing session`);
      existing.ws.close(1000, 'Replaced by new connection');
    }
    this.#byId.set(client.id, client);
  }

  get(id: string): ClientSession | undefined {
    return this.#byId.get(id);
  }

  getAll(): ClientSession[] {
    return Array.from(this.#byId.values());
  }

  has(id: string): boolean {
    return this.#byId.has(id);
  }

  delete(id: string): void {
    this.#byId.delete(id);
  }
}
