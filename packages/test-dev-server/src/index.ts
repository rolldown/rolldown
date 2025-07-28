import chokidar from 'chokidar';
import connect from 'connect';
import nodeFs from 'node:fs';
import http from 'node:http';
import nodePath from 'node:path';
import nodeUrl from 'node:url';
import * as rolldown from 'rolldown';
import serveStatic from 'serve-static';
import { WebSocket, WebSocketServer } from 'ws';
import {
  decodeClientMessageFrom,
  HmrInvalidateMessage,
} from './client-message.js';
import { createDevServerPlugin } from './create-dev-server-plugin.js';
import {
  defineDevConfig,
  DevConfig,
  ServeOptions,
} from './define-dev-config.js';

let seed = 0;

async function loadDevConfig(): Promise<DevConfig> {
  const exports = await import(
    nodeUrl.pathToFileURL(nodePath.join(process.cwd(), 'dev.config.mjs'))
      .href
  );
  return exports.default;
}

export async function serve(): Promise<void> {
  console.log('Starting dev server...');
  const devConfig = await loadDevConfig();

  const buildOptions = devConfig.build ?? {};
  if (buildOptions.plugins == null || Array.isArray(buildOptions.plugins)) {
    buildOptions.plugins = [
      ...(buildOptions.plugins || []),
      createDevServerPlugin(),
    ];
  } else {
    throw new Error('Plugins must be an array');
  }

  console.log('Build options:', buildOptions);

  const connectServer = connect();

  const server = http.createServer(connectServer);
  const wsServer = new WebSocketServer({ server });

  const build = await rolldown.rolldown(buildOptions);

  const devServer = new DevServer(
    buildOptions,
    devConfig.serve ?? {},
    connectServer,
    server,
    wsServer,
    build,
  );
  await devServer.serve();
}

class DevServer {
  private numberOfLiveConnections = 0;
  private sockets: Set<WebSocket> = new Set();
  private currentBuildingPromise: Promise<rolldown.RolldownOutput> | null =
    null;

  constructor(
    private buildOptions: rolldown.BuildOptions,
    private serveOptions: ServeOptions,
    private connectServer: connect.Server,
    private httpServer: http.Server,
    private wsServer: WebSocketServer,
    private build: rolldown.RolldownBuild,
  ) {
  }

  async serve() {
    this.wsServer.on('connection', (ws, _req) => {
      this.numberOfLiveConnections += 1;
      console.debug(
        `Detected new Websocket connection. Current live connections: ${this.numberOfLiveConnections}`,
      );
      this.sockets.add(ws);
      ws.on('error', console.error);
      ws.on('close', () => {
        this.numberOfLiveConnections -= 1;
        console.debug(
          `Websocket connection closed. Current live connections: ${this.numberOfLiveConnections}`,
        );
      });
      ws.on('message', (message) => {
        const clientMessage = decodeClientMessageFrom(message.toString());
        switch (clientMessage.type) {
          case 'hmr:invalidate':
            this.handleHmrInvalidate(this.build, clientMessage);
            break;
        }
      });
    });
    const initialBuildPromise = this.triggerBuild();
    this.connectServer.use(async function(req, _res, next) {
      if (req.url === '/' || req.url === '/index.html') {
        console.info('Detected requests. Waiting for initial build...');
        await initialBuildPromise;
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

    this.httpServer.listen(3000, () => {
      console.log(`Serving ${nodePath.join(process.cwd(), 'dist')}`);
      console.log('Server listening on http://localhost:3000');
    });

    const watcher = chokidar.watch(nodePath.join(process.cwd(), 'src'), {
      usePolling: true,
      interval: 100,
    });

    watcher.on('change', async (path) => {
      console.log(`File ${path} has been changed`);
      if (this.hasLiveConnections) {
        const output = (await this.build.generateHmrPatch([path]))!;
        this.sendUpdateToClient(output);
      }
      // Invoke the build process again to ensure if users reload the page, they get the latest changes
      await this.triggerBuild();
    });
  }

  get hasLiveConnections() {
    return this.numberOfLiveConnections > 0;
  }

  async triggerBuild() {
    if (this.currentBuildingPromise != null) {
      await this.currentBuildingPromise;
    }
    const buildingPromise = this.build.write(this.buildOptions.output).finally(
      () => {
        if (this.currentBuildingPromise === buildingPromise) {
          this.currentBuildingPromise = null;
        }
      },
    );
    this.currentBuildingPromise = buildingPromise;
    await buildingPromise;
  }

  get isInBuilding() {
    return this.currentBuildingPromise != null;
  }

  sendMessage(message: UpdateMessage) {
    if (this.hasLiveConnections) {
      for (const s of this.sockets) {
        if (s.readyState === WebSocket.OPEN) {
          s.send(JSON.stringify(message));
        }
      }
    }
  }

  sendUpdateToClient(
    output: Awaited<ReturnType<rolldown.RolldownBuild['hmrInvalidate']>>,
  ) {
    if (this.hasLiveConnections && output.code) {
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
      this.sendMessage({
        type: 'update',
        url: patchUriForBrowser,
        path: patchUriForFile,
      });
    } else {
      console.debug(
        `Failed to send update to client due to ${
          JSON.stringify(
            {
              hasLiveConnections: this.hasLiveConnections,
              hasCode: output.code != null,
            },
            null,
            2,
          )
        }`,
      );
    }
  }

  async handleHmrInvalidate(
    build: rolldown.RolldownBuild,
    msg: HmrInvalidateMessage,
  ) {
    console.log('Invalidating...');
    if (this.hasLiveConnections) {
      const output = await build.hmrInvalidate(msg.moduleId);
      this.sendUpdateToClient(output);
    }
  }
}

export { defineDevConfig };

interface UpdateMessage {
  type: 'update';
  url: string;
  path: string;
}
