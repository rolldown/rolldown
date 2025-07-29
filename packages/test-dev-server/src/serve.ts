import connect from 'connect';
import http from 'node:http';
import * as rolldown from 'rolldown';
import { WebSocketServer } from 'ws';
import { DevServer } from './dev-server.js';
import { createDevServerPlugin } from './utils/create-dev-server-plugin.js';
import { loadDevConfig } from './utils/load-dev-config.js';
import { normalizeDevOptions } from './utils/normalize-dev-options.js';

export async function serve(): Promise<void> {
  console.log('Starting dev server...');
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

  console.log('Build options:', buildOptions);

  const connectServer = connect();

  const server = http.createServer(connectServer);
  const wsServer = new WebSocketServer({ server });

  const build = await rolldown.rolldown(buildOptions);

  const devServer = new DevServer(
    buildOptions,
    devOptions,
    connectServer,
    server,
    wsServer,
    build,
  );
  await devServer.serve();
}
