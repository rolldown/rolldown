import type { BindingHmrUpdate, DevEngine } from 'rolldown/experimental';
import { dev } from 'rolldown/experimental';
import type { WebSocket } from 'ws';
import { Clients } from '../clients.js';
import { ClientSession } from '../types/client-session.js';
import type { Logger } from '../types/logger.js';
import type { ConnectedMessage, HmrUpdateMessage } from '../types/server-message.js';
import { getDevWatchOptionsForCi } from '../utils/get-dev-watch-options-for-ci.js';
import { withResolvers } from '../utils/with-resolvers.js';

type ServerMessage = HmrUpdateMessage | ConnectedMessage;

export interface FullBundleDevEnvironmentOptions {
  inputOptions: Parameters<typeof dev>[0];
  outputOptions: Parameters<typeof dev>[1];
  /** Sink for server-side log output. Defaults to `console`. */
  logger?: Logger;
}

/**
 * The **node-platform** dev environment: owns the dev engine and the connected
 * clients for fixtures whose artifact is written to disk and executed as a
 * child process by the fixture harness. Build errors surface through the
 * server logs (`Build error:` / `HMR update error:`), which is what the node
 * fixtures assert on.
 * HMR updates are pushed to every client, and the harness restarts the artifact:
 * the default runtime has no boundary walk, so it never pulls a patch.
 *
 * Browser configs run on Vite's own full bundle mode instead (see
 * `../vite-server.ts`). Vite's bundled dev is client-environment-only, so the
 * node platform needs this custom engine wiring.
 */
export class FullBundleDevEnvironment {
  readonly logger: Logger;

  #devEngine!: DevEngine;
  #clients: Clients;

  /**
   * Resolved once the first `onOutput` callback (success or error) has
   * executed on the JS side. `run()` resolves when the initial build settles
   * inside the engine, which can race the callback dispatch; awaiting this
   * guarantees the first bundle is actually on disk.
   */
  #firstOutput = withResolvers<void>();

  // Test-only instrumentation (no Vite equivalent); surfaced via the status
  // middleware so the fixture harness can await builds.
  #buildSeq = 0;

  private constructor(logger: Logger) {
    this.logger = logger;
    this.#clients = new Clients(logger);
  }

  /** Create the environment and its dev engine. */
  static async create(options: FullBundleDevEnvironmentOptions): Promise<FullBundleDevEnvironment> {
    const env = new FullBundleDevEnvironment(options.logger ?? console);
    env.#devEngine = await dev(options.inputOptions, options.outputOptions, {
      onHmrUpdates: (result) => env.#onHmrUpdates(result),
      onOutput: (result) => env.#onOutput(result),
      watch: { ...getDevWatchOptionsForCi(), skipWrite: false },
    });
    return env;
  }

  /** Run the initial build. */
  async run(): Promise<void> {
    const start = Date.now();
    this.logger.info('Starting initial build...');
    await this.#devEngine.run();
    this.logger.info(`Initial build completed in ${Date.now() - start}ms`);
  }

  async close(): Promise<void> {
    await this.#devEngine.close();
  }

  /** Resolves after the first build output (or build error) reached the JS side. */
  async waitForFirstOutput(): Promise<void> {
    return this.#firstOutput.promise;
  }

  triggerFullBuild(): void {
    this.#devEngine.triggerFullBuild();
  }

  async getStatus(): Promise<{
    hasStaleOutput: boolean;
    lastBuildErrored: boolean;
    buildSeq: number;
    connectedClients: number;
  }> {
    const bundleState = await this.#devEngine.getBundleState();
    return {
      hasStaleOutput: bundleState.hasStaleOutput,
      lastBuildErrored: bundleState.lastBuildErrored,
      buildSeq: this.#buildSeq,
      connectedClients: this.#clients.size,
    };
  }

  // --- Client lifecycle (driven by the DevServer's websocket transport) ------

  /** Register a freshly-connected client and ack it. */
  connectClient(ws: WebSocket, clientId: string): ClientSession {
    const client = new ClientSession(ws, clientId);
    this.#clients.setupIfNeeded(client);
    this.#send(ws, { type: 'connected' });
    return client;
  }

  disconnectClient(clientId: string): void {
    this.#clients.delete(clientId);
  }

  // --- Dev engine callbacks --------------------------------------------------

  #onHmrUpdates(result: Error | { update: BindingHmrUpdate; changedFiles: string[] }): void {
    if (result instanceof Error) {
      this.logger.error('HMR update error:', result);
      this.#buildSeq++;
      return;
    }
    const { update, changedFiles } = result;
    if (changedFiles.length > 0 && update.type === 'Changed') {
      for (const client of this.#clients.getAll()) {
        this.#send(client.ws, {
          type: 'hmr:update',
          changedIds: update.changedIds,
          staleIds: update.staleIds,
          gen: update.gen,
        });
      }
      // This platform is restart-based: the harness restarts the artifact, so refresh the
      // on-disk dist. The engine leaves the bundle output stale after an HMR update.
      this.#devEngine.ensureLatestBuildOutput();
    }
    this.#buildSeq++;
  }

  #onOutput(result: Error | { output: readonly unknown[] }): void {
    this.#firstOutput.resolve();
    if (result instanceof Error) {
      this.logger.error('Build error:', result);
      this.#buildSeq++;
      return;
    }
    // A fresh full bundle is written to disk by the engine. The fixture
    // harness restarts the artifact itself, so no client-facing reload
    // signal is needed here.
    this.#buildSeq++;
  }

  // --- Messaging -------------------------------------------------------------

  #send(socket: WebSocket, message: ServerMessage): void {
    if (socket.readyState === socket.OPEN) {
      socket.send(JSON.stringify(message));
    }
  }
}
