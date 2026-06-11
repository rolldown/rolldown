import nodeFs from 'node:fs';
import nodePath from 'node:path';
import nodeUrl from 'node:url';
import type { BindingClientHmrUpdate, DevEngine } from 'rolldown/experimental';
import { dev } from 'rolldown/experimental';
import type { WebSocket } from 'ws';
import { Clients } from '../clients.js';
import { MemoryFiles, weakEtag } from '../memory-files.js';
import { ClientSession } from '../types/client-session.js';
import type {
  BuildOkMessage,
  ConnectedMessage,
  ErrorMessage,
  HmrReloadMessage,
  HmrUpdateMessage,
} from '../types/server-message.js';
import { debounce } from '../utils/debounce.js';
import { getDevWatchOptionsForCi } from '../utils/get-dev-watch-options-for-ci.js';
import { prepareError } from '../utils/prepare-error.js';

type ServerMessage =
  | HmrUpdateMessage
  | HmrReloadMessage
  | ConnectedMessage
  | ErrorMessage
  | BuildOkMessage;

let seed = 0;

export interface FullBundleDevEnvironmentOptions {
  inputOptions: Parameters<typeof dev>[0];
  outputOptions: Parameters<typeof dev>[1];
  /**
   * Serve from memory (Vite full-bundle parity) vs. disk. Derived from the
   * rolldown build target — see `DevServer`.
   */
  serveFromMemory: boolean;
}

/**
 * Port of Vite's `FullBundleDevEnvironment`
 * (`packages/vite/src/node/server/environments/fullBundleEnvironment.ts`).
 *
 * Owns the dev engine, the in-memory output store, the connected clients, and
 * the build-error / reload bookkeeping. The surrounding `DevServer` owns the
 * http/ws transport and wires connections + middlewares into this environment —
 * the same split Vite has between `server/index.ts` and this class. The only
 * intentional divergences are documented inline: a `node` build target serves
 * from disk (the fixture harness execs the artifact), and the client transport
 * is the rolldown default HMR runtime over a plain websocket rather than Vite's
 * hot channel.
 */
export class FullBundleDevEnvironment {
  /**
   * In-memory output store (Vite parity). On a browser target the engine runs
   * with `watch.skipWrite: true` so the full bundle and HMR patches live here
   * and are served by the memory-files / index-html middlewares. Empty on a
   * node target, which serves the on-disk `dist/`.
   */
  readonly memoryFiles = new MemoryFiles();
  readonly serveFromMemory: boolean;

  #devEngine!: DevEngine;
  #clients = new Clients();

  /**
   * Most recent build error from *either* callback channel. Set in both
   * `onOutput` and `onHmrUpdates` on failure, cleared on a success from either
   * channel, and replayed to freshly-connected clients so the error survives a
   * refresh. See `meta/design/dev-engine.md` §2.
   */
  #lastBuildError: Error | null = null;
  /**
   * A FullReload HMR update defers its page reload until the auto-upgraded
   * rebuild lands in `onOutput`, avoiding an error-overlay flash on a build
   * that turns out to break.
   */
  #fullReloadPending = false;
  /** Gate for access-triggered regeneration (see `triggerBundleRegenerationIfStale`). */
  #initialBuildCompleted = false;

  // Test-only instrumentation (no Vite equivalent); surfaced via the status
  // middleware so the e2e harness can await builds / module registrations.
  #buildSeq = 0;
  #moduleRegistrationSeq = 0;

  // Debounced broadcast reload, mirroring Vite's `debouncedFullReload`.
  // Only browser clients act on a reload; node artifact processes are restarted
  // by the test harness instead, so we skip them.
  #debouncedFullReload = debounce(20, () => {
    if (!this.serveFromMemory) return;
    for (const client of this.#clients.getAll()) {
      this.#send(client.ws, { type: 'hmr:reload' });
    }
    console.log('[hmr]: page reload');
  });

  private constructor(serveFromMemory: boolean) {
    this.serveFromMemory = serveFromMemory;
  }

  /** Create the environment and its dev engine (Vite's `listen()`). */
  static async create(options: FullBundleDevEnvironmentOptions): Promise<FullBundleDevEnvironment> {
    const env = new FullBundleDevEnvironment(options.serveFromMemory);
    env.#devEngine = await dev(options.inputOptions, options.outputOptions, {
      onHmrUpdates: (result) => env.#onHmrUpdates(result),
      onOutput: (result) => env.#onOutput(result),
      watch: { ...getDevWatchOptionsForCi(), skipWrite: options.serveFromMemory },
    });
    return env;
  }

  /** Run the initial build, then reload any spinner clients (Vite parity). */
  async run(): Promise<void> {
    const start = Date.now();
    console.log('Starting initial build...');
    await this.#devEngine.run();
    console.log(`Initial build completed in ${Date.now() - start}ms`);
    // `run()` resolves once the initial build settles (success OR failure), so
    // the engine is now in its steady state and access-triggered regeneration
    // may begin.
    this.#initialBuildCompleted = true;
    // Reload any "Bundling in progress" spinner clients that connected during
    // the initial build (Vite broadcasts a full-reload after the first build).
    this.#debouncedFullReload();
  }

  async close(): Promise<void> {
    this.memoryFiles.clear();
    await this.#devEngine.close();
  }

  triggerFullBuild(): void {
    this.#devEngine.triggerFullBuild();
  }

  async getStatus(): Promise<{
    hasStaleOutput: boolean;
    lastBuildErrored: boolean;
    buildSeq: number;
    connectedClients: number;
    moduleRegistrationSeq: number;
  }> {
    const bundleState = await this.#devEngine.getBundleState();
    return {
      hasStaleOutput: bundleState.hasStaleOutput,
      lastBuildErrored: bundleState.lastBuildErrored,
      buildSeq: this.#buildSeq,
      connectedClients: this.#clients.size,
      moduleRegistrationSeq: this.#moduleRegistrationSeq,
    };
  }

  // --- Client lifecycle (driven by the DevServer's websocket transport) ------

  /** Register a freshly-connected client; ack it and replay any cached error. */
  connectClient(ws: WebSocket, clientId: string): ClientSession {
    const client = new ClientSession(ws, clientId);
    this.#clients.setupIfNeeded(client);

    this.#send(ws, { type: 'connected' });
    // Replay the cached build error so it survives a browser refresh (Vite
    // `vite:client:connect` parity).
    if (this.#lastBuildError) {
      this.#sendError(ws, this.#lastBuildError);
    }
    return client;
  }

  async disconnectClient(clientId: string): Promise<void> {
    this.#clients.delete(clientId);
    await this.#devEngine.removeClient(clientId);
  }

  async registerModules(clientId: string, modules: string[]): Promise<void> {
    console.log('Registering modules:', modules);
    await this.#devEngine.registerModules(clientId, modules);
    this.#moduleRegistrationSeq++;
  }

  /**
   * Programmatic `import.meta.hot.invalidate()` (Vite's `invalidateModule`).
   * Surfaces an invalidate-time build error to the calling client instead of
   * crashing the connection handler.
   */
  async invalidate(moduleId: string, client: ClientSession): Promise<void> {
    console.log('Invalidating...');
    let updates: BindingClientHmrUpdate[];
    try {
      updates = await this.#devEngine.invalidate(moduleId);
    } catch (e) {
      const error = e as Error;
      this.#lastBuildError = error;
      this.#sendError(client.ws, error);
      return;
    }
    // `invalidate()` never auto-upgrades to a rebuild, so onOutput won't fire;
    // FullReload updates must trigger regeneration + reload inline.
    this.#handleHmrUpdates(updates, true);
  }

  // --- Access-triggered regeneration / lazy bundling (Vite parity) -----------

  /**
   * Vite's `triggerBundleRegenerationIfStale` with the conservative-rebuild
   * guard (dev-engine §1/§12): regenerate only when output is stale, the last
   * build did NOT error, and the initial build has completed. Fire-and-forget —
   * kick the rebuild, reload when it lands, and return whether a regeneration
   * was triggered so the caller can serve the spinner meanwhile.
   *
   * Exception (dev-engine §3/§12): after an HMR-stage failure a page access
   * forces a full rebuild instead, so a buggy HMR generation can be recovered
   * by reloading. A Rebuild-stage or full-build failure is left alone.
   */
  async triggerBundleRegenerationIfStale(): Promise<boolean> {
    const state = await this.#devEngine.getBundleState();

    // Trigger full build if the HMR errors,
    // this is to make it easier to recover if the HMR generation is broken for some reason.
    if (this.#initialBuildCompleted && state.lastBuildErrored && state.lastErrorStage === 'Hmr') {
      this.#devEngine.triggerFullBuild();
      this.#devEngine.ensureLatestBuildOutput().then(() => this.#debouncedFullReload());
      return true;
    }

    const shouldTrigger =
      state.hasStaleOutput && !state.lastBuildErrored && this.#initialBuildCompleted;
    if (shouldTrigger) {
      this.#devEngine.ensureLatestBuildOutput().then(() => this.#debouncedFullReload());
    }
    return shouldTrigger;
  }

  /** Compile a lazy entry on demand (Vite's `triggerLazyBundling`). */
  async triggerLazyBundling(
    moduleId: string | null,
    clientId: string | null,
  ): Promise<string | undefined> {
    if (!moduleId || !clientId) {
      return undefined;
    }
    return this.#devEngine.compileEntry(moduleId, clientId);
  }

  // --- Dev engine callbacks --------------------------------------------------

  #onHmrUpdates(
    result: Error | { updates: BindingClientHmrUpdate[]; changedFiles: string[] },
  ): void {
    if (result instanceof Error) {
      console.error('HMR update error:', result);
      this.#lastBuildError = result;
      this.#broadcastError(result);
      this.#buildSeq++;
      return;
    }
    // A successful HMR computation supersedes any cached error.
    const wasErrored = this.#lastBuildError !== null;
    this.#lastBuildError = null;
    if (wasErrored) {
      // Recovered from a build error via HMR. The recovery patch reaches only
      // the module's own client; broadcast `build:ok` so the error overlay
      // clears on every client (incl. the separate overlay client).
      this.#broadcast({ type: 'build:ok' });
    }
    const { updates, changedFiles } = result;
    const hasFullReload = updates.some((u) => u.update.type === 'FullReload');
    // Mirror Vite: skip client-facing work for empty / all-noop batches.
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
    if (result instanceof Error) {
      console.error('Build error:', result);
      this.#lastBuildError = result;
      this.#broadcastError(result);
      this.#buildSeq++;
      return;
    }
    // A fresh full bundle clears any cached error. Remember whether we are
    // recovering from one: a plain recovery rebuild emits no client-facing HMR
    // message, so we must reload to clear the error overlay / spinner and show
    // the now-working page.
    const wasErrored = this.#lastBuildError !== null;
    this.#lastBuildError = null;
    if (this.serveFromMemory) {
      // Mirror Vite: populate the in-memory output. Don't clear first —
      // incremental builds reuse files. Lazily materialize per request.
      for (const outputFile of result.output as Array<
        | { type: 'chunk'; fileName: string; code: string }
        | { type: 'asset'; fileName: string; source: string | Uint8Array }
      >) {
        const fileName = outputFile.fileName;
        this.memoryFiles.set(fileName, () => {
          const source = outputFile.type === 'chunk' ? outputFile.code : outputFile.source;
          return { source, etag: weakEtag(source) };
        });
      }
    }
    // Fire a deferred full reload (queued by a FullReload HMR update), or a
    // recovery reload (a build that just went from errored → ok), now that
    // fresh output exists — so we never reload onto a broken bundle.
    if (this.#fullReloadPending || wasErrored) {
      this.#fullReloadPending = false;
      this.#debouncedFullReload();
    }
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
            console.warn(`Client ${clientUpdate.clientId} not found`);
            continue;
          }
          this.#sendPatch(client.ws, update);
          break;
        }
        case 'FullReload':
          if (fromInvalidate) {
            // An invalidate-driven FullReload does not auto-upgrade to a
            // rebuild, so onOutput won't fire — regenerate then reload here.
            this.#devEngine.ensureLatestBuildOutput().then(() => {
              this.#debouncedFullReload();
            });
          } else {
            // Defer the reload until the auto-upgraded rebuild lands in
            // onOutput (avoids flashing onto a possibly-broken bundle).
            this.#fullReloadPending = true;
          }
          break;
        case 'Noop':
          console.warn(`Client ${clientUpdate.clientId} received noop update`);
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
      console.debug('Failed to send update to client: patch has no code');
      return;
    }
    console.log('Patching...');

    if (this.serveFromMemory) {
      // Store the patch (and its sourcemap) in memory; the client loads it by
      // URL via the memory-files middleware. The `\n; export {}` guard mirrors
      // Vite — it forces the patch to be treated as an ES module to mitigate
      // XSSI-style attacks (see fullBundleEnvironment.ts).
      this.memoryFiles.set(output.filename, { source: output.code + '\n; export {}' });
      if (output.sourcemapFilename && output.sourcemap) {
        this.memoryFiles.set(output.sourcemapFilename, { source: output.sourcemap });
      }
      const url = `/${output.filename}`;
      this.#send(socket, { type: 'hmr:update', url, path: url });
      return;
    }

    // node: write the patch to disk; the artifact imports it via a file:// URL.
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

  #broadcast(message: ServerMessage): void {
    for (const client of this.#clients.getAll()) {
      this.#send(client.ws, message);
    }
  }

  /** Push a build error to every connected client (Vite's `prepareError` broadcast). */
  #broadcastError(error: Error): void {
    this.#broadcast({ type: 'error', err: prepareError(error) });
  }

  #sendError(socket: WebSocket, error: Error): void {
    this.#send(socket, { type: 'error', err: prepareError(error) });
  }

  #send(socket: WebSocket, message: ServerMessage): void {
    if (socket.readyState === socket.OPEN) {
      socket.send(JSON.stringify(message));
    }
  }
}
