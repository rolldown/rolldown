import connect from 'connect';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import nodePath from 'node:path';
import serveStatic from 'serve-static';
import { WebSocketServer } from 'ws';
import { FullBundleDevEnvironment } from './environments/full-bundle-dev-environment.js';
import { statusMiddleware } from './middlewares/status.js';
import { decodeClientMessage } from './utils/decode-client-message.js';
import type { Logger } from './types/logger.js';
import type { DevConfig } from './utils/define-dev-config.js';
import { loadDevConfig } from './utils/load-dev-config.js';
import { normalizeDevOptions } from './utils/normalize-dev-options.js';
import { createViteDevServer } from './vite-server.js';
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
 * The http/websocket transport around a {@link FullBundleDevEnvironment} for
 * the **node platform only**: the artifact (and its HMR patches) are written
 * to disk, executed as a child process by the fixture harness, and `dist/` is
 * served statically.
 *
 * The browser platform runs on Vite's own full bundle mode instead
 * (`experimental.bundledDev`, see `vite-server.ts`). Vite's bundled dev is
 * client-environment-only, which is why this node transport (disk output,
 * request gate, stdin rebuild trigger) is a custom implementation.
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

  // node gates requests until the initial build is on disk.
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
    // Shallow-clone before injecting the dev-server plumbing so starting a
    // server never mutates the caller's config (a config object may be used
    // to create more than one server).
    const buildOptions = { ...this.#config.build };

    if (buildOptions.plugins != null && !Array.isArray(buildOptions.plugins)) {
      throw new Error('Plugins must be an array');
    }

    // Bind BEFORE building: with `port: 0` the OS-assigned port only exists
    // after listen, and the HMR runtime's websocket address is baked into the
    // bundle at build time via `experimental.devMode.port`.
    // See internal-docs/dev-server-test-harness/implementation.md ("listen-before-build").
    this.#prepareGate();
    this.#port = await this.#listen(this.#options.port);

    // Inject the bound port into devMode options for the HMR runtime.
    const experimental = { ...buildOptions.experimental };
    const devMode = experimental.devMode ?? {};
    experimental.devMode = typeof devMode === 'object' ? { ...devMode, port: this.#port } : devMode;
    buildOptions.experimental = experimental;

    const { output: outputOptions, ...inputOptions } = buildOptions;

    try {
      const env = await FullBundleDevEnvironment.create({
        inputOptions,
        outputOptions: outputOptions ?? {},
        logger: this.#logger,
      });
      this.#env = env;
      this.#prepareWebSocket(env);
      if (this.#options.attachStdinTrigger) {
        this.#prepareStdin(env);
      }
      this.#registerMiddlewares(env);

      await env.run();
      // `run()` resolves when the initial build settles inside the engine, but
      // the JS-side output callback may not have executed yet. Wait for it so
      // a resolved `start()` means the first bundle (or its error) is on disk.
      await env.waitForFirstOutput();
    } catch (e) {
      // Engine/lifecycle failure after the socket is bound: release the port
      // so the error surfaces instead of a leaked listener keeping the
      // process alive. (Build errors don't throw — they arrive via the
      // engine callbacks; see internal-docs/dev-engine/implementation.md §16.)
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

  /** First middleware: block requests until the initial build is on disk. */
  #prepareGate(): void {
    this.#connectServer.use(async (_req, _res, next) => {
      if (this.#serverStatus.allowRequest) {
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

  #registerMiddlewares(env: FullBundleDevEnvironment): void {
    this.#connectServer.use(statusMiddleware(env));
    // The artifact (and HMR patches) are written to disk and read directly by
    // the fixture harness, so serve `dist/` statically.
    this.#connectServer.use(
      serveStatic(nodePath.join(process.cwd(), 'dist'), {
        index: ['index.html'],
        extensions: ['html'],
      }),
    );
  }

  #readyHttpServer(): void {
    this.#serverStatus.allowRequest = true;
    this.#serverStatus.resolvers.resolve();
  }
}

/**
 * Programmatic entry point for the test harness: take a config, bind an
 * OS-assigned port (or `opts.port`), run the initial build, and hand back the
 * resolved URL plus a `close()`.
 *
 * Dispatches on the build target: a `browser` build runs on Vite's full
 * bundle mode (`vite-server.ts`); anything else runs the node disk-serving
 * transport above.
 *
 * Deliberately does NOT consult `DEV_SERVER_PORT`: that env var is the
 * fixtures/CLI channel consumed by `serve()`.
 */
export async function createDevServer(
  config: DevConfig,
  opts?: { port?: number; logger?: Logger; attachStdinTrigger?: boolean },
): Promise<DevServerHandle> {
  if (config.build?.platform === 'browser') {
    // Browser configs run on Vite bundled dev. (No stdin rebuild trigger on
    // this path — that channel exists for the node fixtures only.)
    return createViteDevServer(config, { port: opts?.port, logger: opts?.logger });
  }
  const devServer = new DevServer(config, {
    port: opts?.port ?? 0,
    attachStdinTrigger: opts?.attachStdinTrigger ?? false,
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
  await createDevServer(devConfig, { port, attachStdinTrigger: true });
}
