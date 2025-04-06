import chokidar from 'chokidar';
import connect from 'connect';
import nodeFs from 'node:fs';
import http from 'node:http';
import nodePath from 'node:path';
import * as rolldown from 'rolldown';
import serveStatic from 'serve-static';
import { WebSocket, WebSocketServer } from 'ws';
import { createDevServerPlugin } from './create-dev-server-plugin.js';
import { defineDevConfig, DevConfig } from './define-dev-config.js';

let seed = 0;

async function loadDevConfig(): Promise<DevConfig> {
  const exports = await import(nodePath.join(process.cwd(), 'dev.config.mjs'));
  return exports.default;
}

class DevServer {
  private config: DevConfig;
  private numberOfLiveConnections = 0;

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
    const watcher = chokidar.watch(nodePath.join(process.cwd(), 'src'));

    app.use(
      serveStatic(nodePath.join(process.cwd(), 'dist'), {
        index: ['index.html'],
        extensions: ['html'],
      }),
    );

    // create node.js http server and listen on port
    const server = http.createServer(app);
    const wsServer = new WebSocketServer({ server });
    let socket: WebSocket;
    wsServer.on('connection', (ws) => {
      this.numberOfLiveConnections += 1;
      console.debug(
        `Detected new Websocket connection. Current live connections: ${this.numberOfLiveConnections}`,
      );
      socket = ws;
      ws.on('error', console.error);
    });
    wsServer.on('close', () => {
      this.numberOfLiveConnections -= 1;
      console.debug(
        `Detected Websocket disconnection. Current live connections: ${this.numberOfLiveConnections}`,
      );
    });
    watcher.on('change', async (path) => {
      console.log(`File ${path} has been changed`);
      if (this.hasLiveConnections) {
        const patch = await build.generateHmrPatch([path]);
        if (patch) {
          console.log('Patching...');
          if (socket) {
            const path = `${seed}.js`;
            seed++;
            nodeFs.writeFileSync(
              nodePath.join(process.cwd(), 'dist', path),
              patch,
            );
            console.log(
              'Patch:',
              JSON.stringify({
                type: 'update',
                url: path,
              }),
            );
            socket.send(
              JSON.stringify({
                type: 'update',
                url: path,
              }),
            );
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
}

export async function serve(): Promise<void> {
  console.log('Starting dev server...');
  const devConfig = await loadDevConfig();
  const devServer = new DevServer(devConfig);
  await devServer.serve();
}

export { defineDevConfig };
