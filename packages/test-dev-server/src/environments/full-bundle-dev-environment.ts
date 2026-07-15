import nodeFs from 'node:fs';
import nodePath from 'node:path';
import nodeUrl from 'node:url';
import type { BindingClientHmrUpdate, DevEngine } from 'rolldown/experimental';
import { dev } from 'rolldown/experimental';
import type { WebSocket } from 'ws';
import { Clients } from '../clients.js';
import { ClientSession } from '../types/client-session.js';
import type { Logger } from '../types/logger.js';
import type { ConnectedMessage, HmrUpdateMessage } from '../types/server-message.js';
import { getDevWatchOptionsForCi } from '../utils/get-dev-watch-options-for-ci.js';
import { withResolvers } from '../utils/with-resolvers.js';

type ServerMessage = HmrUpdateMessage | ConnectedMessage;

let seed = 0;

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
 * HMR patches are written to `dist/` as numbered files and imported by the
 * artifact via `file://` URLs.
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
    // create the server-side session (fresh `shipped[C]` ship map) used to select the
    // factories each patch ships to this client
    void this.#devEngine.registerClient(clientId);

    this.#send(ws, { type: 'connected' });
    return client;
  }

  async disconnectClient(clientId: string): Promise<void> {
    this.#clients.delete(clientId);
    await this.#devEngine.removeClient(clientId);
  }

  // --- Dev engine callbacks --------------------------------------------------

  #onHmrUpdates(
    result: Error | { updates: BindingClientHmrUpdate[]; changedFiles: string[] },
  ): void {
    if (result instanceof Error) {
      this.logger.error('HMR update error:', result);
      this.#buildSeq++;
      return;
    }
    const { updates, changedFiles } = result;
    const hasFullReload = updates.some((u) => u.update.type === 'FullReload');
    // Skip client-facing work for empty / all-noop batches.
    if (changedFiles.length > 0 && !updates.every((u) => u.update.type === 'Noop')) {
      this.#handleHmrUpdates(updates);
    }
    // Only increment if no FullReload — a FullReload triggers a rebuild which
    // will call onOutput, so we let onOutput do the increment to avoid
    // double-counting a single build cycle.
    if (!hasFullReload) {
      this.#buildSeq++;
    }
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

  // --- HMR fan-out -----------------------------------------------------------

  #handleHmrUpdates(updates: BindingClientHmrUpdate[], fromInvalidate = false): void {
    for (const clientUpdate of updates) {
      const update = clientUpdate.update;
      switch (update.type) {
        case 'Patch': {
          const client = this.#clients.get(clientUpdate.clientId);
          if (!client) {
            this.logger.warn(`Client ${clientUpdate.clientId} not found`);
            continue;
          }
          this.#sendPatch(client.ws, update);
          break;
        }
        case 'FullReload':
          if (fromInvalidate) {
            // An invalidate-driven FullReload does not auto-upgrade to a
            // rebuild, so onOutput won't fire — regenerate the on-disk output
            // here. (The artifact process is restarted by the harness; there
            // is no browser page to reload.)
            void this.#devEngine.ensureLatestBuildOutput();
          }
          // Otherwise the auto-upgraded rebuild lands in onOutput, which
          // writes the fresh dist.
          break;
        case 'Noop':
          this.logger.warn(`Client ${clientUpdate.clientId} received noop update`);
          break;
        default:
          throw new Error(`Unknown update type: ${JSON.stringify(update)}`);
      }
    }
  }

  #sendPatch(socket: WebSocket, output: BindingClientHmrUpdate['update']): void {
    if (output.type !== 'Patch') {
      return;
    }
    if (!output.code) {
      this.logger.debug('Failed to send update to client: patch has no code');
      return;
    }
    this.logger.info('Patching...');

    // Write the patch to disk; the artifact imports it via a file:// URL.
    const path = `${seed}.js`;
    seed++;
    nodeFs.writeFileSync(nodePath.join(process.cwd(), 'dist', path), output.code);
    const patchUriForBrowser = `/${path}`;
    const patchUriForFile = nodeUrl
      .pathToFileURL(nodePath.join(process.cwd(), 'dist', path))
      .toString();
    this.#send(socket, {
      type: 'hmr:update',
      url: patchUriForBrowser,
      path: patchUriForFile,
    });
  }

  // --- Messaging -------------------------------------------------------------

  #send(socket: WebSocket, message: ServerMessage): void {
    if (socket.readyState === socket.OPEN) {
      socket.send(JSON.stringify(message));
    }
  }
}
