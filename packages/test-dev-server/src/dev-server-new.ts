import connect from 'connect';
import http from 'node:http';
import nodePath from 'node:path';
import { dev, DevEngine } from 'rolldown/experimental';
import serveStatic from 'serve-static';
import { WebSocketServer } from 'ws';
import { createDevServerPlugin } from './utils/create-dev-server-plugin.js';
import { loadDevConfig } from './utils/load-dev-config.js';
import { normalizeDevOptions } from './utils/normalize-dev-options.js';

class DevServer {
  connectServer = connect();
  server = http.createServer(this.connectServer);
  serverStatus = {
    allowRequest: false,
    allowRequestPromiseResolvers: Promise.withResolvers<void>(),
  };
  wsServer = new WebSocketServer({ server: this.server });

  constructor() {}

  async serve(): Promise<void> {
    this.#prepareHttpServer();

    const devConfig = await loadDevConfig();
    const devOptions = normalizeDevOptions(devConfig.dev ?? {});
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
    let devEngine = await dev(inputOptions, outputOptions ?? {});
    this.#prepareHttpServerAfterCreateDevEngine(devEngine);
    await devEngine.run();
    this.#readyHttpServer();
  }

  #prepareHttpServer(): void {
    this.connectServer.use(async (_req, _res, next) => {
      if (this.serverStatus.allowRequest) {
        next();
      } else {
        await this.serverStatus.allowRequestPromiseResolvers.promise;
        next();
      }
    });

    this.server.listen(3000, () => {
      console.log('Server listening on http://localhost:3000');
    });
  }

  #prepareHttpServerAfterCreateDevEngine(devEngine: DevEngine): void {
    this.connectServer.use(async (req, _res, next) => {
      if (req.url === '/' || req.url === '/index.html') {
        await devEngine.ensureCurrentBuildFinish();
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
}

export async function serveNew(): Promise<void> {
  const devServer = new DevServer();
  await devServer.serve();
}
