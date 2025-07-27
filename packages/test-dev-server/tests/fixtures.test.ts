import { execa, ExecaError } from 'execa';
import glob from 'fast-glob';
import killPort from 'kill-port';
import nodeFs from 'node:fs';
import nodeFsPromise from 'node:fs/promises';
import nodePath from 'node:path';
import { afterAll, beforeAll, describe, test } from 'vitest';
import { removeDirSync } from './src/utils';

function main() {
  const fixturesPath = nodePath.resolve(__dirname, 'fixtures');
  const tmpFixturesPath = nodePath.resolve(__dirname, 'tmp/fixtures');

  async function updateNodeModules() {
    await execa('pnpm install --no-frozen-lockfile', {
      cwd: fixturesPath,
      shell: true,
      stdio: 'inherit',
    });
  }

  removeDirSync(tmpFixturesPath);
  // Copy project to temp directory. Remember to filter out `node_modules` and `dist` directories
  nodeFs.mkdirSync(tmpFixturesPath, { recursive: true });
  nodeFs.cpSync(fixturesPath, tmpFixturesPath, {
    recursive: true,
    filter: (src) => {
      return !src.includes('node_modules') && !src.includes('dist');
    },
  });

  beforeAll(async () => {
    await updateNodeModules();
  }, 30 * 1000);

  afterAll(async () => {
    if (!process.env.CI) {
      console.log('🔄 - Cleaning up tmp/fixtures directory...');
      console.log('🔄 - Resetting node_modules...');
      removeDirSync(tmpFixturesPath);
      await updateNodeModules();
      console.log('✅ - Cleanup completed');
    }
  }, 30 * 1000);

  const fixtureNames = nodeFs.readdirSync(fixturesPath);
  describe('fixtures', () => {
    for (const fixtureName of fixtureNames) {
      test.sequential(`fixture: ${fixtureName}`, async () => {
        const projectName = fixtureName;
        const tmpProjectPath = nodePath.join(
          tmpFixturesPath,
          projectName,
        );

        await killPort(3000).catch(err =>
          console.debug(`kill-port: ${err?.message}`)
        ); // Kill any process running on port 3000

        const devServeProcess = execa('pnpm serve', {
          cwd: tmpProjectPath,
          shell: true,
          stdio: 'inherit',
          env: {
            RUST_BACKTRACE: 'FULL',
            RD_LOG: 'hmr=debug',
          },
        });

        await ensurePathExists(nodePath.join(tmpProjectPath, 'dist/main.js'));

        const nodeScriptPath = nodePath.join(tmpProjectPath, 'dist/main.js');

        console.log('🔄 Starting Node.js process: ', nodeScriptPath);
        const runningArtifactProcess = execa(
          `node ${nodeScriptPath}`,
          { cwd: tmpProjectPath, shell: true, stdio: 'inherit' },
        );

        await new Promise<void>((rsl, _rej) => {
          setTimeout(rsl, 5000);
        });

        console.log('🔄 Collecting HMR edit files...');
        const hmrEditFiles = await collectHmrEditFiles(tmpProjectPath);

        console.log('🔄 Processing HMR edit files...');
        for (const hmrEditFile of hmrEditFiles) {
          console.log(`🔄 Processing HMR edit file: ${hmrEditFile.path}`);
          const newContent = await nodeFsPromise.readFile(
            hmrEditFile.path,
            'utf-8',
          );
          await nodeFsPromise.writeFile(hmrEditFile.targetPath, newContent);
          console.log(
            `📝 Written content to: ${hmrEditFile.targetPath}`,
          );
          console.log(
            `⏳ Waiting for HMR to be triggered... ${hmrEditFile.targetPath}`,
          );
          await ensurePathExists(nodePath.join(tmpProjectPath, 'ok-1'));
          console.log(
            `✅ Successfully triggered HMR ${hmrEditFile.targetPath}`,
          );
        }

        const catchRunningArtifactProcess = runningArtifactProcess.catch(
          err => {
            if (err instanceof ExecaError && err.signal === 'SIGTERM') {
              console.log(
                'Process killed normally with SIGTERM, ignoring error.',
              );
            } else {
              throw err;
            }
          },
        );

        const catchDevServeProcess = devServeProcess.catch(err => {
          if (err instanceof ExecaError && err.signal === 'SIGTERM') {
            console.log(
              'Process killed normally with SIGTERM, ignoring error.',
            );
          } else {
            throw err;
          }
        });

        runningArtifactProcess.kill('SIGTERM');
        await catchRunningArtifactProcess;

        devServeProcess.kill('SIGTERM');
        await catchDevServeProcess;
      });
    }
  });
}

function ensurePathExists(path: string, timeout = 10000) {
  const startTime = Date.now();
  const isTimeout = () => Date.now() - startTime > timeout;
  return new Promise<void>((resolve, reject) => {
    function check() {
      try {
        nodeFs.accessSync(path);
        console.debug(`Path ${path} exists`);
        resolve();
      } catch (err) {
        if (isTimeout()) {
          const parentDir = nodePath.dirname(path);
          let listedFiles: string[] | null = null;
          if (nodeFs.existsSync(parentDir)) {
            listedFiles = nodeFs.readdirSync(parentDir);
          }
          reject(
            new Error(
              `Path ${path} does not exist after ${timeout}ms. Parent directory contents: ${
                listedFiles?.join(', ')
              }`,
              { cause: err },
            ),
          );
        } else {
          setTimeout(check, 250);
        }
      }
    }
    check();
  });
}

main();

interface HmrEditFile {
  path: string;
  targetPath: string;
  step: number;
}

async function collectHmrEditFiles(
  projectPath: string,
): Promise<HmrEditFile[]> {
  const hmrEditFiles = await glob(
    nodePath.join(projectPath, '/src/**/*.hmr-*.*'),
    {
      ignore: ['**/node_modules/**', '**/dist/**'],
      absolute: true,
    },
  );
  const files = hmrEditFiles.map((path): HmrEditFile => {
    // path: /xxx/xxx/example.hmr-1.js

    // .js
    const ext = nodePath.extname(path);

    // example.hmr-1
    const basenameWithoutExt = nodePath.basename(path, ext);

    // 1
    const step = parseInt(basenameWithoutExt.slice(
      basenameWithoutExt.lastIndexOf('-') + 1,
    ));

    const originalBasename = basenameWithoutExt.slice(
      0,
      basenameWithoutExt.lastIndexOf('.'),
    );

    // /xxx/xxx/example.js
    const targetPath = nodePath.join(
      nodePath.dirname(path),
      originalBasename,
    ) + ext;

    return {
      path,
      targetPath,
      step: step,
    };
  });

  return files.sort((a, b) => a.step - b.step);
}
