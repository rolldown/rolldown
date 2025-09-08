import connect from 'connect';
import nodeFs from 'node:fs';
import http from 'node:http';
import nodePath from 'node:path';
import nodeUrl from 'node:url';
import * as rolldown from 'rolldown';
import { dev, DevEngine } from 'rolldown/experimental';
import serveStatic from 'serve-static';
import { WebSocket, WebSocketServer } from 'ws';
import { HmrInvalidateMessage } from './types/client-message.js';
import { NormalizedDevOptions } from './types/normalized-dev-options.js';
import { HmrUpdateMessage } from './types/server-message.js';
import { createDevServerPlugin } from './utils/create-dev-server-plugin.js';
import { decodeClientMessage } from './utils/decode-client-message.js';
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
  #sockets = new Set<WebSocket>();
  #devOptions?: NormalizedDevOptions;
  #devEngine?: DevEngine;

  constructor() {}

  async serve(): Promise<void> {
    this.#prepareServer();

    const devConfig = await loadDevConfig();
    const devOptions = normalizeDevOptions(devConfig.dev ?? {});
    this.#devOptions = devOptions;
    const buildOptions = devConfig.build ?? {};
    if (buildOptions.plugins == null || Array.isArray(buildOptions.plugins)) {
      buildOptions.plugins = [
        ...(buildOptions.plugins || []),
        createDevServerPlugin(devOptions),
      ];
    } else {
      throw new Error('Plugins must be an array');
    }

    const { output: outputOptions, ...inputOptions } = buildOptions;
    let devEngine = await dev(inputOptions, outputOptions ?? {}, {
      onHmrUpdates: (updates) => {
        this.handleHmrUpdates(updates);
      },
      watch: {
        usePolling: true,
        pollInterval: 50,
      },
    });
    this.#devEngine = devEngine;
    this.#prepareHttpServerAfterCreateDevEngine(devEngine);
    await devEngine.run();
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
      this.#sockets.add(ws);
      ws.on('error', console.error);
      ws.on('close', () => {
        // TODO: handle close
      });
      ws.on('message', async (rawData) => {
        const clientMessage = decodeClientMessage(rawData);
        switch (clientMessage.type) {
          case 'hmr:invalidate':
            await this.#handleHmrInvalidate(clientMessage);
            break;
        }
      });
    });

    this.server.listen(3000, () => {
      console.log('Server listening on http://localhost:3000');
    });
  }

  #prepareHttpServerAfterCreateDevEngine(devEngine: DevEngine): void {
    this.connectServer.use(async (req, _res, next) => {
      if (req.url === '/' || req.url === '/index.html') {
        await devEngine.ensureLatestBuild();
        next();
      } else {
        next();
      }
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

  #sendMessage(message: HmrUpdateMessage): void {
    if (this.#sockets.size > 0) {
      const encoded = JSON.stringify(message);
      for (const s of this.#sockets) {
        if (s.readyState === WebSocket.OPEN) {
          s.send(encoded);
        }
      }
    }
  }

  handleHmrUpdates(
    updates: Awaited<ReturnType<rolldown.RolldownBuild['generateHmrPatch']>>,
  ): void {
    for (const update of updates) {
      switch (update.type) {
        case 'Patch':
          this.sendUpdateToClient(update);
          break;
        case 'FullReload':
          if (this.#devOptions?.platform === 'browser') {
            // TODO: send reload message to client
          }
          break;
        case 'Noop':
          break;
        default:
          throw new Error(`Unknown update type: ${update}`);
      }
    }
  }

  sendUpdateToClient(
    output: Awaited<ReturnType<rolldown.RolldownBuild['hmrInvalidate']>>,
  ): void {
    if (output.type !== 'Patch') {
      return;
    }
    if (output.code) {
      console.log('Patching...');
      const path = `${seed}.js`;
      seed++;
      nodeFs.writeFileSync(
        nodePath.join(process.cwd(), 'dist', path),
        output.code,
      );
      const patchUriForBrowser = `/${path}`;
      const patchUriForFile = nodeUrl.pathToFileURL(
        nodePath.join(process.cwd(), 'dist', path),
      ).toString();
      console.log(
        'Patch:',
        JSON.stringify({
          type: 'update',
          url: patchUriForBrowser,
          path: patchUriForFile,
        }),
      );
      this.#sendMessage({
        type: 'hmr:update',
        url: patchUriForBrowser,
        path: patchUriForFile,
      });
    } else {
      console.debug(
        `Failed to send update to client due to ${
          JSON.stringify(
            {
              hasCode: output.code != null,
            },
            null,
            2,
          )
        }`,
      );
    }
  }

  async #handleHmrInvalidate(
    msg: HmrInvalidateMessage,
  ): Promise<void> {
    console.log('Invalidating...');
    if (this.#sockets.size > 0) {
      const output = await this.#devEngine!.invalidate(msg.moduleId);
      this.sendUpdateToClient(output);
    }
  }
}

export async function serve(): Promise<void> {
  const devServer = new DevServer();
  await devServer.serve();
}
