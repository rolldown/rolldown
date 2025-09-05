import { execa, ExecaError } from 'execa';
import glob from 'fast-glob';
import killPort from 'kill-port';
import nodeFs from 'node:fs';
import nodePath from 'node:path';
import { afterAll, beforeAll, describe, test } from 'vitest';
import { removeDirSync, sensibleTimeoutInMs } from './src/utils';

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

        // Wait for the Node.js process to start
        await sensibleTimeoutInMs(1000);

        console.log('🔄 Collecting HMR edit files...');
        const hmrEditFiles = await collectHmrEditFiles(tmpProjectPath);

        console.log('🔄 Processing HMR edit files...');
        for (const [index, [step, hmrEdits]] of hmrEditFiles.entries()) {
          // Make sure the poll-based watcher could detect the change (poll interval is 100ms)
          // Files in the same step will be edited in the same timeframe.
          await sensibleTimeoutInMs(200);
          for (const hmrEdit of hmrEdits) {
            console.log(
              `🔄 Processing HMR edit file: step ${step} - ${hmrEdit.replacementPath}`,
            );
            const newContent = nodeFs.readFileSync(
              hmrEdit.replacementPath,
              'utf-8',
            );
            nodeFs.writeFileSync(hmrEdit.targetPath, newContent);
            console.log(
              `📝 Written content to: ${hmrEdit.targetPath}`,
            );
          }
          console.log(
            `⏳ Waiting for HMR to be triggered for step ${step}`,
          );
          await ensurePathExists(
            nodePath.join(tmpProjectPath, `ok-${index}`),
            10 * 1000,
          );
          console.log(
            `✅ Successfully triggered HMR for step ${step} with ${
              JSON.stringify(hmrEdits, null, 2)
            }`,
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

function ensurePathExists(path: string, timeout = 6 * 1000) {
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
  replacementPath: string;
  targetPath: string;
  step: number;
}

async function collectHmrEditFiles(
  projectPath: string,
) {
  const hmrEditFiles = await glob(
    nodePath.join(projectPath, '/src/**/*.hmr-*.*'),
    {
      ignore: ['**/node_modules/**', '**/dist/**'],
      absolute: true,
    },
  );
  const files = hmrEditFiles.map((replacementPath): HmrEditFile => {
    // path: /xxx/xxx/example.hmr-1.js

    // .js
    const ext = nodePath.extname(replacementPath);

    // example.hmr-1
    const basenameWithoutExt = nodePath.basename(replacementPath, ext);

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
      nodePath.dirname(replacementPath),
      originalBasename,
    ) + ext;

    return {
      replacementPath,
      targetPath,
      step: step,
    };
  });
  // Group files by step (Map.groupBy is not available in Node 20)
  const filesByStep = new Map<number, HmrEditFile[]>();
  for (const file of files) {
    const stepFiles = filesByStep.get(file.step) || [];
    stepFiles.push(file);
    filesByStep.set(file.step, stepFiles);
  }
  const stepFiles = [...filesByStep.entries()];
  stepFiles.sort(([aStep], [bStep]) => aStep - bStep);
  return stepFiles;
}
