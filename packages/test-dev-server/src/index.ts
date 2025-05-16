import chokidar from 'chokidar';
import connect from 'connect';
import nodeFs from 'node:fs';
import http from 'node:http';
import nodePath from 'node:path';
import nodeUrl from 'node:url';
import * as rolldown from 'rolldown';
import serveStatic from 'serve-static';
import { WebSocket, WebSocketServer } from 'ws';
import { createDevServerPlugin } from './create-dev-server-plugin.js';
import { defineDevConfig, DevConfig } from './define-dev-config.js';

let seed = 0;

async function loadDevConfig(): Promise<DevConfig> {
  const exports = await import(
    nodeUrl.pathToFileURL(nodePath.join(process.cwd(), 'dev.config.mjs'))
      .href
  );
  return exports.default;
}

class DevServer {
  private config: DevConfig;
  private numberOfLiveConnections = 0;
  private sockets: Set<WebSocket> = new Set();

  constructor(config: DevConfig) {
    this.config = config;
  }

  get hasLiveConnections() {
    return this.numberOfLiveConnections > 0;
  }

  async serve() {
    const buildOptions = this.config.build ?? {};
    if (buildOptions.plugins == null || Array.isArray(buildOptions.plugins)) {
      buildOptions.plugins = [
        ...(buildOptions.plugins || []),
        createDevServerPlugin(),
      ];
    } else {
      throw new Error('Plugins must be an array');
    }
    // buildOptions.write = true
    console.log('Build options:', buildOptions);
    // const build = await rolldown.build(buildOptions)
    const build = await rolldown.rolldown(buildOptions);
    await build.write(buildOptions.output);

    const app = connect();

    console.log(`Serving ${nodePath.join(process.cwd(), 'dist')}`);
    const watcher = chokidar.watch(nodePath.join(process.cwd(), 'src'), {
      usePolling: true,
      interval: 100,
    });

    app.use(
      serveStatic(nodePath.join(process.cwd(), 'dist'), {
        index: ['index.html'],
        extensions: ['html'],
      }),
    );

    // create node.js http server and listen on port
    const server = http.createServer(app);
    const wsServer = new WebSocketServer({ server });
    wsServer.on('connection', (ws, req) => {
      const url = new URL(req.url!, `http://${req.headers.host}`);
      const from = url.searchParams.get('from');
      if (from === 'hmr-runtime') {
        this.sendMessage({ type: 'connected-from-hmr-runtime' });
      }

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
    });
    watcher.on('change', async (path) => {
      console.log(`File ${path} has been changed`);
      if (this.hasLiveConnections) {
        const output = (await build.generateHmrPatch([path]))!;
        if (output.code) {
          console.log('Patching...');
          if (this.hasLiveConnections) {
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
            console.log('No socket connected');
          }
        } else {
          console.log('No patch found');
        }
      }
      // Invoke the build process again to ensure if users reload the page, they get the latest changes
      await build.write(buildOptions.output);
    });
    server.listen(3000);
    console.log('Server listening on http://localhost:3000');
  }

  sendMessage(message: ConnectedFromHmrRuntime | UpdateMessage) {
    if (this.hasLiveConnections) {
      for (const s of this.sockets) {
        if (s.readyState === WebSocket.OPEN) {
          s.send(JSON.stringify(message));
        }
      }
    }
  }
}

export async function serve(): Promise<void> {
  console.log('Starting dev server...');
  const devConfig = await loadDevConfig();
  const devServer = new DevServer(devConfig);
  await devServer.serve();
}

export { defineDevConfig };

interface ConnectedFromHmrRuntime {
  type: 'connected-from-hmr-runtime';
}

interface UpdateMessage {
  type: 'update';
  url: string;
  path: string;
}
