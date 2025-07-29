import chokidar from 'chokidar';
import connect from 'connect';
import nodeFs from 'node:fs';
import http from 'node:http';
import nodePath from 'node:path';
import nodeUrl from 'node:url';
import * as rolldown from 'rolldown';
import serveStatic from 'serve-static';
import { WebSocket, WebSocketServer } from 'ws';
import { HmrInvalidateMessage } from './types/client-message.js';
import { NormalizedDevOptions } from './types/normalized-dev-options.js';
import { UpdateMessage } from './types/server-message.js';
import { decodeClientMessage } from './utils/decode-client-message.js';

let seed = 0;

export class DevServer {
  private numberOfLiveConnections = 0;
  private sockets: Set<WebSocket> = new Set();
  private currentBuildingPromise: Promise<rolldown.RolldownOutput> | null =
    null;

  constructor(
    private buildOptions: rolldown.BuildOptions,
    private devOptions: NormalizedDevOptions,
    private connectServer: connect.Server,
    private httpServer: http.Server,
    private wsServer: WebSocketServer,
    private build: rolldown.RolldownBuild,
  ) {
  }

  async serve(): Promise<void> {
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
        const clientMessage = decodeClientMessage(message.toString());
        switch (clientMessage.type) {
          case 'hmr:invalidate':
            this.handleHmrInvalidate(this.build, clientMessage);
            break;
        }
      });
    });
    const initialBuildPromise = this.triggerBuild();
    if (this.devOptions.platform === 'browser') {
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
    }

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

  get hasLiveConnections(): boolean {
    return this.numberOfLiveConnections > 0;
  }

  async triggerBuild(): Promise<void> {
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

  get isInBuilding(): boolean {
    return this.currentBuildingPromise != null;
  }

  sendMessage(message: UpdateMessage): void {
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
  ): void {
    if (output.type !== 'Patch') {
      return;
    }
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
  ): Promise<void> {
    console.log('Invalidating...');
    if (this.hasLiveConnections) {
      const output = await build.hmrInvalidate(msg.moduleId);
      this.sendUpdateToClient(output);
    }
  }
}
