import connect from 'connect';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import nodePath from 'node:path';
import serveStatic from 'serve-static';
import { WebSocketServer } from 'ws';
import { FullBundleDevEnvironment } from './environments/full-bundle-dev-environment.js';
import { indexHtmlMiddleware } from './middlewares/index-html.js';
import { memoryFilesMiddleware } from './middlewares/memory-files.js';
import { statusMiddleware } from './middlewares/status.js';
import { triggerLazyBundlingMiddleware } from './middlewares/trigger-lazy-bundling.js';
import { createDevServerPlugin } from './utils/create-dev-server-plugin.js';
import { decodeClientMessage } from './utils/decode-client-message.js';
import type { Logger } from './types/logger.js';
import type { DevConfig } from './utils/define-dev-config.js';
import { loadDevConfig } from './utils/load-dev-config.js';
import { normalizeDevOptions } from './utils/normalize-dev-options.js';
import { withResolvers } from './utils/with-resolvers.js';

interface DevServerOptions {
  /** Port to bind. `0` lets the OS assign one (the default for `createDevServer`). */
  port: number;
  /**
   * Wire the stdin `'r'` rebuild trigger. Only the CLI path (`serve()`) wants
   * this — the node fixtures signal rebuilds through it. In-process servers
   * created by the test harness must not touch the worker's stdin.
   */
  attachStdinTrigger: boolean;
  /** Sink for server-side log output. Defaults to `console`. */
  logger?: Logger;
}

export interface DevServerHandle {
  /** The resolved URL of the running server, bound port included. */
  url: string;
  port: number;
  close(): Promise<void>;
}

/**
 * The http/websocket transport and middleware wiring around a
 * {@link FullBundleDevEnvironment} — the test-dev-server equivalent of Vite's
 * `server/index.ts`. All full-bundle behavior lives in the environment; this
 * class only owns the connect/http/ws plumbing and registers the same
 * middleware chain Vite does.
 */
class DevServer {
  #connectServer = connect();
  #server = http.createServer(this.#connectServer);
  #wsServer = new WebSocketServer({ server: this.#server });
  #config: DevConfig;
  #options: DevServerOptions;
  #env: FullBundleDevEnvironment | null = null;
  #logger: Logger;
  #stdinListener: ((data: Buffer) => void) | null = null;
  #port = 0;
  #closed = false;

  // node platform gates requests until the initial build is on disk (browser
  // serves a spinner instead, so it never blocks). Mirrors nothing in Vite —
  // Vite is browser-only.
  #serverStatus = {
    allowRequest: false,
    resolvers: withResolvers<void>(),
  };

  constructor(config: DevConfig, options: DevServerOptions) {
    this.#config = config;
    this.#options = options;
    this.#logger = options.logger ?? console;
  }

  async start(): Promise<DevServerHandle> {
    const devOptions = normalizeDevOptions(this.#config.dev ?? {});
    // Shallow-clone before injecting the dev-server plumbing so starting a
    // server never mutates the caller's config (a config object may be used
    // to create more than one server).
    const buildOptions = { ...this.#config.build };

    // Serve from memory (Vite full-bundle parity) only for a browser build
    // target; node builds keep disk serving (the fixture harness execs the
    // artifact from disk). `build.platform` is the only platform signal set
    // consistently across every fixture/playground config.
    const serveFromMemory = buildOptions.platform === 'browser';

    if (buildOptions.plugins != null && !Array.isArray(buildOptions.plugins)) {
      throw new Error('Plugins must be an array');
    }

    // Bind BEFORE building: with `port: 0` the OS-assigned port only exists
    // after listen, and the HMR runtime's websocket address is baked into the
    // bundle at build time via `experimental.devMode.port`.
    // See meta/design/dev-server-test-harness.md ("listen-before-build").
    this.#prepareGate(serveFromMemory);
    this.#port = await this.#listen(this.#options.port);

    // Inject the bound port into devMode options for the HMR runtime.
    const experimental = { ...buildOptions.experimental };
    const devMode = experimental.devMode ?? {};
    experimental.devMode = typeof devMode === 'object' ? { ...devMode, port: this.#port } : devMode;
    buildOptions.experimental = experimental;
    buildOptions.plugins = [
      ...(buildOptions.plugins ?? []),
      createDevServerPlugin(devOptions, this.#logger),
    ];

    const { output: outputOptions, ...inputOptions } = buildOptions;

    try {
      const env = await FullBundleDevEnvironment.create({
        inputOptions,
        outputOptions: outputOptions ?? {},
        serveFromMemory,
        logger: this.#logger,
      });
      this.#env = env;
      this.#prepareWebSocket(env);
      if (this.#options.attachStdinTrigger) {
        this.#prepareStdin(env);
      }
      this.#registerMiddlewares(env, serveFromMemory);

      await env.run();
      // `run()` resolves when the initial build settles inside the engine, but
      // the JS-side output callback may not have executed yet. Wait for it so
      // a resolved `start()` means the first bundle (or its error) is being
      // served — a navigation will not land on the spinner.
      await env.waitForFirstOutput();
    } catch (e) {
      // Engine/lifecycle failure after the socket is bound: release the port
      // so the error surfaces instead of a leaked listener keeping the
      // process alive. (Build errors don't throw — they arrive via the
      // engine callbacks; see meta/design/dev-engine.md §16.)
      await this.close().catch(() => {});
      throw e;
    }
    this.#readyHttpServer();

    return {
      url: `http://localhost:${this.#port}`,
      port: this.#port,
      close: () => this.close(),
    };
  }

  /**
   * The teardown the subprocess model never needed (it SIGKILLed instead):
   * stop accepting websocket connections, drop connected clients, close the
   * http server, then close the dev engine so its watcher and worker threads
   * release the process.
   */
  async close(): Promise<void> {
    if (this.#closed) {
      return;
    }
    this.#closed = true;
    if (this.#stdinListener) {
      process.stdin.off('data', this.#stdinListener);
      this.#stdinListener = null;
    }
    this.#wsServer.close();
    for (const client of this.#wsServer.clients) {
      client.terminate();
    }
    // Destroy idle keep-alive sockets so `close()` resolves promptly.
    this.#server.closeAllConnections();
    if (this.#server.listening) {
      await new Promise<void>((resolve, reject) => {
        this.#server.close((err) => (err ? reject(err) : resolve()));
      });
    }
    await this.#env?.close();
  }

  async #listen(port: number): Promise<number> {
    await new Promise<void>((resolve, reject) => {
      this.#server.once('error', reject);
      this.#server.listen(port, () => {
        this.#server.off('error', reject);
        resolve();
      });
    });
    const boundPort = (this.#server.address() as AddressInfo).port;
    this.#logger.info(`Server listening on http://localhost:${boundPort}`);
    return boundPort;
  }

  /** First middleware: block node requests until the initial build is ready. */
  #prepareGate(serveFromMemory: boolean): void {
    this.#connectServer.use(async (_req, _res, next) => {
      // Browser never blocks: a not-ready request is answered with the
      // "Bundling in progress" spinner by the index-html middleware.
      if (serveFromMemory || this.#serverStatus.allowRequest) {
        next();
      } else {
        await this.#serverStatus.resolvers.promise;
        next();
      }
    });
  }

  #prepareWebSocket(env: FullBundleDevEnvironment): void {
    this.#wsServer.on('connection', (ws, req) => {
      const url = new URL(req.url ?? '', `http://localhost:${this.#port}`);
      const clientId = url.searchParams.get('clientId');
      if (!clientId) {
        this.#logger.warn('WebSocket connection without clientId, closing');
        ws.close(1008, 'Missing clientId');
        return;
      }

      const client = env.connectClient(ws, clientId);

      ws.on('error', (err) => this.#logger.error(err));
      ws.on('close', () => {
        env.disconnectClient(client.id);
        this.#logger.info(`Client ${client.id} disconnected`);
      });
      ws.on('message', async (rawData) => {
        const clientMessage = decodeClientMessage(rawData);
        switch (clientMessage.type) {
          case 'hmr:invalidate':
            await env.invalidate(clientMessage.moduleId, client);
            break;
          case 'hmr:module-registered':
            await env.registerModules(client.id, clientMessage.modules);
            break;
          default: {
            const _never: never = clientMessage;
            void _never;
          }
        }
      });
    });
  }

  #prepareStdin(env: FullBundleDevEnvironment): void {
    this.#stdinListener = (data) => {
      if (data.toString() === 'r') {
        env.triggerFullBuild();
      }
    };
    process.stdin.on('data', this.#stdinListener);
    // Unref stdin to prevent it from keeping the process alive.
    // Some Node.js versions (e.g., 24) may not have unref() on stdin.
    if (typeof process.stdin.unref === 'function') {
      process.stdin.unref();
    }
  }

  /** Register the full-bundle middleware chain (Vite's `server/index.ts` order). */
  #registerMiddlewares(env: FullBundleDevEnvironment, serveFromMemory: boolean): void {
    this.#connectServer.use(triggerLazyBundlingMiddleware(env));
    this.#connectServer.use(statusMiddleware(env));

    if (serveFromMemory) {
      this.#connectServer.use(memoryFilesMiddleware(env));
      this.#connectServer.use(indexHtmlMiddleware(env));
    } else {
      // node: the artifact (and HMR patches) are written to disk and read
      // directly by the fixture harness, so serve `dist/` statically.
      this.#connectServer.use(
        serveStatic(nodePath.join(process.cwd(), 'dist'), {
          index: ['index.html'],
          extensions: ['html'],
        }),
      );
    }
  }

  #readyHttpServer(): void {
    this.#serverStatus.allowRequest = true;
    this.#serverStatus.resolvers.resolve();
  }
}

/**
 * Programmatic entry point for the browser test harness: take a config, bind
 * an OS-assigned port (or `opts.port`), run the initial build, and hand back
 * the resolved URL plus a `close()` — the test-dev-server analog of Vite's
 * `createServer(config).listen()` / `server.resolvedUrls`.
 *
 * Deliberately does NOT consult `DEV_SERVER_PORT`: that env var is the
 * fixtures/CLI channel consumed by `serve()`.
 */
export async function createDevServer(
  config: DevConfig,
  opts?: { port?: number; logger?: Logger },
): Promise<DevServerHandle> {
  const devServer = new DevServer(config, {
    port: opts?.port ?? 0,
    attachStdinTrigger: false,
    logger: opts?.logger,
  });
  return devServer.start();
}

/** CLI entry point (`serve` bin): config and port come from cwd / env. */
export async function serve(): Promise<void> {
  const devConfig = await loadDevConfig(process.cwd());
  const devOptions = normalizeDevOptions(devConfig.dev ?? {});
  const port = process.env.DEV_SERVER_PORT
    ? parseInt(process.env.DEV_SERVER_PORT, 10)
    : devOptions.port;
  const devServer = new DevServer(devConfig, { port, attachStdinTrigger: true });
  await devServer.start();
}
