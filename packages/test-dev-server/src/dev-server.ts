import connect from 'connect';
import nodeFs from 'node:fs';
import http from 'node:http';
import nodePath from 'node:path';
import nodeUrl from 'node:url';
import type { BindingClientHmrUpdate, DevEngine } from 'rolldown/experimental';
import { dev } from 'rolldown/experimental';
import serveStatic from 'serve-static';
import { WebSocket, WebSocketServer } from 'ws';
import type { HmrInvalidateMessage } from './types/client-message.js';
import { ClientSession } from './types/client-session.js';
import type { NormalizedDevOptions } from './types/normalized-dev-options.js';
import type {
  ConnectedMessage,
  HmrReloadMessage,
  HmrUpdateMessage,
} from './types/server-message.js';
import { createDevServerPlugin } from './utils/create-dev-server-plugin.js';
import { decodeClientMessage } from './utils/decode-client-message.js';
import { getDevWatchOptionsForCi } from './utils/get-dev-watch-options-for-ci.js';
import { loadDevConfig } from './utils/load-dev-config.js';
import { normalizeDevOptions } from './utils/normalize-dev-options.js';

let seed = 0;

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

class DevServer {
  connectServer = connect();
  server = http.createServer(this.connectServer);
  serverStatus = {
    allowRequest: false,
    allowRequestPromiseResolvers: withResolvers<void>(),
  };
  wsServer = new WebSocketServer({ server: this.server });
  #clients = new Map<string, ClientSession>();
  #devOptions?: NormalizedDevOptions;
  #devEngine?: DevEngine;
  #port = 3000;

  constructor() {}

  #sendMessage(
    socket: WebSocket,
    message: HmrUpdateMessage | HmrReloadMessage | ConnectedMessage,
  ): void {
    if (socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify(message));
    }
  }

  async serve(): Promise<void> {
    const devConfig = await loadDevConfig();
    const devOptions = normalizeDevOptions(devConfig.dev ?? {});
    this.#devOptions = devOptions;
    this.#port = process.env.DEV_SERVER_PORT
      ? parseInt(process.env.DEV_SERVER_PORT, 10)
      : devOptions.port;

    this.#prepareServer();

    const buildOptions = devConfig.build ?? {};

    // Inject port into devMode options for HMR runtime
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
    let devEngine = await dev(inputOptions, outputOptions ?? {}, {
      onHmrUpdates: (errOrUpdates) => {
        if (errOrUpdates instanceof Error) {
          console.error('HMR update error:', errOrUpdates);
        } else {
          this.handleHmrUpdates(errOrUpdates.updates);
        }
      },
      onOutput: (errOrOutputs) => {
        if (errOrOutputs instanceof Error) {
          console.error('Build error:', errOrOutputs);
        }
      },
      watch: getDevWatchOptionsForCi(),
    });
    this.#devEngine = devEngine;
    process.stdin
      .on('data', async (data) => {
        if (data.toString() === 'r') {
          const { hasStaleOutput } = await devEngine.getBundleState();
          if (hasStaleOutput) {
            await devEngine.ensureLatestBuildOutput();
          }
        }
      })
      .unref();
    this.#prepareHttpServerAfterCreateDevEngine(devEngine);
    const initialBuildStart = Date.now();
    console.log('Starting initial build...');
    await devEngine.run();
    const initialBuildEnd = Date.now();
    console.log(`Initial build completed in ${initialBuildEnd - initialBuildStart}ms`);
    this.#readyHttpServer();
  }

  #prepareServer(): void {
    this.connectServer.use(async (_req, _res, next) => {
      if (this.serverStatus.allowRequest) {
        next();
      } else {
        await this.serverStatus.allowRequestPromiseResolvers.promise;
        next();
      }
    });

    this.wsServer.on('connection', (ws, _req) => {
      const clientSession = new ClientSession(ws);
      this.#clients.set(clientSession.id, clientSession);

      // Send the client its assigned ID so it can use it for lazy compilation requests
      this.#sendMessage(ws, { type: 'connected', clientId: clientSession.id });

      ws.on('error', console.error);
      ws.on('close', () => {
        this.#clients.delete(clientSession.id);
        this.#devEngine?.removeClient(clientSession.id);
        console.log(`Client ${clientSession.id} disconnected`);
      });
      ws.on('message', async (rawData) => {
        const clientMessage = decodeClientMessage(rawData);
        switch (clientMessage.type) {
          case 'hmr:invalidate':
            await this.#handleHmrInvalidate(clientMessage);
            break;
          case 'hmr:module-registered': {
            console.log('Registering modules:', clientMessage.modules);
            this.#devEngine?.registerModules(clientSession.id, clientMessage.modules);
            break;
          }
          default: {
            const _never: never = clientMessage;
          }
        }
      });
    });

    this.server.listen(this.#port, () => {
      console.log(`Server listening on http://localhost:${this.#port}`);
    });
  }

  #prepareHttpServerAfterCreateDevEngine(devEngine: DevEngine): void {
    this.connectServer.use(async (req, _res, next) => {
      if (req.url === '/' || req.url === '/index.html') {
        await devEngine.ensureLatestBuildOutput();
        next();
      } else {
        next();
      }
    });
    this.connectServer.use(async (req, res, next) => {
      if (req.url?.startsWith('/lazy?')) {
        try {
          const url = new URL(req.url, `http://localhost:${this.#port}`);
          const moduleId = url.searchParams.get('id');
          const clientId = url.searchParams.get('clientId');
          console.log(`Lazy compile request for module ${moduleId} from client ${clientId}`);

          if (moduleId && clientId) {
            const moduleCode = await devEngine.compileEntry(moduleId, clientId);
            if (moduleCode != null) {
              res!.setHeader('Content-Type', 'application/javascript');
              res!.end(moduleCode);
              return;
            }
          }
        } catch (err) {
          // Return server error response
          res!.statusCode = 500;
          res!.end('Internal Server Error during lazy compilation');
          console.error('Error handling lazy compile request:', err);
          return;
        }
      }
      next();
    });
    this.connectServer.use(
      serveStatic(nodePath.join(process.cwd(), 'dist'), {
        index: ['index.html'],
        extensions: ['html'],
      }),
    );
  }

  #readyHttpServer() {
    this.serverStatus.allowRequest = true;
    this.serverStatus.allowRequestPromiseResolvers.resolve();
  }

  handleHmrUpdates(updates: BindingClientHmrUpdate[]): void {
    for (const clientUpdate of updates) {
      const update = clientUpdate.update;
      switch (update.type) {
        case 'Patch': {
          const client = this.#clients.get(clientUpdate.clientId);
          if (!client) {
            console.warn(`Client ${clientUpdate.clientId} not found`);
            continue;
          }
          this.sendUpdateToClient(client.ws, update);
          break;
        }
        case 'FullReload':
          if (this.#devOptions?.platform === 'browser') {
            const client = this.#clients.get(clientUpdate.clientId);
            if (!client) {
              console.warn(`Client ${clientUpdate.clientId} not found`);
              break;
            }
            console.log(`[hmr]: Sending reload message to client ${clientUpdate.clientId}`);
            this.#sendMessage(client.ws, { type: 'hmr:reload' });
          }
          this.#devEngine?.ensureLatestBuildOutput();
          break;
        case 'Noop':
          console.warn(`Client ${clientUpdate.clientId} received noop update`);
          break;
        default:
          throw new Error(`Unknown update type: ${update}`);
      }
    }
  }

  sendUpdateToClient(socket: WebSocket, output: BindingClientHmrUpdate['update']): void {
    if (output.type !== 'Patch') {
      return;
    }
    if (output.code) {
      console.log('Patching...');
      const path = `${seed}.js`;
      seed++;
      nodeFs.writeFileSync(nodePath.join(process.cwd(), 'dist', path), output.code);
      const patchUriForBrowser = `/${path}`;
      const patchUriForFile = nodeUrl
        .pathToFileURL(nodePath.join(process.cwd(), 'dist', path))
        .toString();
      console.log(
        'Patch:',
        JSON.stringify({
          type: 'update',
          url: patchUriForBrowser,
          path: patchUriForFile,
        }),
      );
      this.#sendMessage(socket, {
        type: 'hmr:update',
        url: patchUriForBrowser,
        path: patchUriForFile,
      });
    } else {
      console.debug(
        `Failed to send update to client due to ${JSON.stringify(
          {
            hasCode: output.code != null,
          },
          null,
          2,
        )}`,
      );
    }
  }

  async #handleHmrInvalidate(msg: HmrInvalidateMessage): Promise<void> {
    console.log('Invalidating...');
    // Always invalidate - sendMessage will handle empty client lists
    const updates = await this.#devEngine!.invalidate(msg.moduleId);
    this.handleHmrUpdates(updates);
  }
}

export async function serve(): Promise<void> {
  const devServer = new DevServer();
  await devServer.serve();
}
