import { execa } from 'execa';
import nodeFs from 'node:fs/promises';
import nodePath from 'node:path';
import { test } from 'vitest';

import WebSocket from 'ws';

async function waitForWsConnection(
  url: string,
  timeout = 5000,
  interval = 300,
): Promise<void> {
  const start = Date.now();

  return new Promise((resolve, reject) => {
    const tryConnect = () => {
      const socket = new WebSocket(url);

      socket.on('open', () => {
        socket.close();
        resolve();
      });

      socket.on('error', () => {
        if (Date.now() - start > timeout) {
          reject(new Error(`Timeout: Unable to connect to ${url}`));
        } else {
          setTimeout(tryConnect, interval);
        }
      });
    };

    tryConnect();
  });
}

test('basic', async () => {
  const projectPath = nodePath.resolve(__dirname, 'fixtures/basic');
  // Copy project to a temp directory
  const tempPath = nodePath.resolve(__dirname, 'tmp/fixtures/basic');

  // Remove temp directory if exists
  await nodeFs.rm(tempPath, { recursive: true, force: true });
  // Copy project to temp directory. Remember to filter out `node_modules` and `dist` directories
  await nodeFs.mkdir(tempPath, { recursive: true });
  await nodeFs.cp(projectPath, tempPath, {
    recursive: true,
    filter: (src) => {
      return !src.includes('node_modules') && !src.includes('dist');
    },
  });
  await execa('pnpm install', { cwd: tempPath, shell: true, stdio: 'inherit' });
  const devServeProcess = execa('pnpm serve', {
    cwd: tempPath,
    shell: true,
    stdio: 'inherit',
    env: {
      RUST_BACKTRACE: 'FULL',
      RD_LOG: 'hmr=debug',
    },
  });

  await waitForWsConnection('ws://localhost:3000');
  const runningNodeProcess = execa(
    `node ${nodePath.join(tempPath, 'dist/main.js')}`,
    { cwd: tempPath, shell: true, stdio: 'inherit' },
  );
  const nodeExitSuccessfullyPromise = new Promise((rsl, rej) => {
    runningNodeProcess.on('exit', (code) => {
      if (code !== 0) {
        rej(new Error(`runningNodeProcess exited with code ${code}`));
      }
      rsl({});
    });
  });
  // stimulate editing files
  const fileContent = await nodeFs.readFile(
    nodePath.resolve(tempPath, 'src/hmr-boundary.js'),
    'utf-8',
  );
  await nodeFs.writeFile(
    nodePath.resolve(tempPath, 'src/hmr-boundary.js'),
    fileContent.replace(
      'export const value = 0;',
      `import { value as depValue } from './new-dep'
export const value = depValue;`,
    ),
  );
  await nodeExitSuccessfullyPromise;
  devServeProcess.kill('SIGINT');
});
