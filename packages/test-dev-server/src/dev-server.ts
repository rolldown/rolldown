import connect from 'connect';
import http from 'node:http';
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
import { loadDevConfig } from './utils/load-dev-config.js';
import { normalizeDevOptions } from './utils/normalize-dev-options.js';

// Node20 does not support `Promise.withResolvers`
const withResolvers = <T>() => {
  let resolve: (value: T | PromiseLike<T>) => void;
  let reject: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve: resolve!, reject: reject! };
};

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
  #port = 3000;

  // node platform gates requests until the initial build is on disk (browser
  // serves a spinner instead, so it never blocks). Mirrors nothing in Vite —
  // Vite is browser-only.
  #serverStatus = {
    allowRequest: false,
    resolvers: withResolvers<void>(),
  };

  async serve(): Promise<void> {
    const devConfig = await loadDevConfig();
    const devOptions = normalizeDevOptions(devConfig.dev ?? {});
    this.#port = process.env.DEV_SERVER_PORT
      ? parseInt(process.env.DEV_SERVER_PORT, 10)
      : devOptions.port;

    const buildOptions = devConfig.build ?? {};

    // Serve from memory (Vite full-bundle parity) only for a browser build
    // target; node builds keep disk serving (the fixture harness execs the
    // artifact from disk). `build.platform` is the only platform signal set
    // consistently across every fixture/playground config.
    const serveFromMemory = buildOptions.platform === 'browser';

    // Inject port into devMode options for the HMR runtime.
    buildOptions.experimental = buildOptions.experimental ?? {};
    buildOptions.experimental.devMode = buildOptions.experimental.devMode ?? {};
    if (typeof buildOptions.experimental.devMode === 'object') {
      buildOptions.experimental.devMode.port = this.#port;
    }

    if (buildOptions.plugins == null || Array.isArray(buildOptions.plugins)) {
      buildOptions.plugins = [...(buildOptions.plugins || []), createDevServerPlugin(devOptions)];
    } else {
      throw new Error('Plugins must be an array');
    }

    const { output: outputOptions, ...inputOptions } = buildOptions;

    // Gate + websocket transport must be wired before the engine starts so a
    // client can connect while the initial build runs.
    this.#prepareGate(serveFromMemory);
    this.#server.listen(this.#port, () => {
      console.log(`Server listening on http://localhost:${this.#port}`);
    });

    const env = await FullBundleDevEnvironment.create({
      inputOptions,
      outputOptions: outputOptions ?? {},
      serveFromMemory,
    });
    this.#prepareWebSocket(env);
    this.#prepareStdin(env);
    this.#registerMiddlewares(env, serveFromMemory);

    await env.run();
    this.#readyHttpServer();
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
        console.warn('WebSocket connection without clientId, closing');
        ws.close(1008, 'Missing clientId');
        return;
      }

      const client = env.connectClient(ws, clientId);

      ws.on('error', console.error);
      ws.on('close', () => {
        env.disconnectClient(client.id);
        console.log(`Client ${client.id} disconnected`);
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
    process.stdin.on('data', (data) => {
      if (data.toString() === 'r') {
        env.triggerFullBuild();
      }
    });
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

export async function serve(): Promise<void> {
  const devServer = new DevServer();
  await devServer.serve();
}
