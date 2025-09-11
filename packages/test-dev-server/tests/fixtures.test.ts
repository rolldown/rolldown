import { execa, ExecaError } from 'execa';
import glob from 'fast-glob';
import killPort from 'kill-port';
import nodeFs from 'node:fs';
import nodePath from 'node:path';
import { afterAll, describe, test } from 'vitest';
import { CONFIG } from './src/config';
import { removeDirSync, sensibleTimeoutInMs } from './src/utils';

function main() {
  const fixturesPath = nodePath.resolve(__dirname, 'fixtures');
  const tmpFixturesPath = nodePath.resolve(__dirname, 'tmp/fixtures');

  async function updateNodeModules(showOutput = true) {
    await execa('pnpm install --no-frozen-lockfile', {
      cwd: fixturesPath,
      shell: true,
      stdio: showOutput ? 'inherit' : ['pipe', 'pipe', 'inherit'],
    });
  }

  console.log(`🔄 - Cleaning up ${tmpFixturesPath} directory...`);
  removeDirSync(tmpFixturesPath);

  afterAll(async () => {
    if (!process.env.CI) {
      console.log(`🔄 - Cleaning up ${tmpFixturesPath} directory...`);
      removeDirSync(tmpFixturesPath);
      console.log(`🔄 - Resetting node_modules...`);
      await updateNodeModules(false);
      console.log(`✅ - Cleanup completed`);
    }
  }, 30 * 1000);

  const fixtureNames = nodeFs.readdirSync(fixturesPath);
  describe('fixtures', () => {
    for (const fixtureName of fixtureNames) {
      // Skip if it's not a valid fixture
      if (
        !nodeFs.existsSync(
          nodePath.join(fixturesPath, fixtureName, 'package.json'),
        )
      ) {
        continue;
      }

      test.sequential(`fixture: ${fixtureName}`, async () => {
        let tmpProjectPath = nodePath.join(
          tmpFixturesPath,
          fixtureName,
        );
        while (nodeFs.existsSync(tmpProjectPath)) {
          tmpProjectPath = nodePath.join(
            tmpFixturesPath,
            fixtureName + '-retry',
          );
        }

        console.log(
          `🔄 - Copying ${
            nodePath.join(fixturesPath, fixtureName)
          } to ${tmpProjectPath}...`,
        );
        nodeFs.mkdirSync(tmpProjectPath, { recursive: true });
        nodeFs.cpSync(
          nodePath.join(fixturesPath, fixtureName),
          tmpProjectPath,
          {
            recursive: true,
            filter: (src) => {
              return !src.includes('node_modules') && !src.includes('dist');
            },
          },
        );

        console.log(`🔄 - Updating node_modules...`);
        await updateNodeModules(true);

        console.log(`🔄 - Killing any process running on port 3000...`);
        try {
          await killPort(3000);
        } catch (err) {
          if (
            err instanceof Error && err.message.includes('No process running')
          ) {
            console.log(`🔄 - No process running on port 3000`);
          } else {
            throw err;
          }
        }

        console.log(`🔄 - Starting dev server...`);
        const devServeProcess = execa('pnpm serve', {
          cwd: tmpProjectPath,
          shell: true,
          stdio: 'inherit',
          env: {
            RUST_BACKTRACE: 'FULL',
            RD_LOG: 'hmr=debug',
          },
        });

        const nodeScriptPath = nodePath.join(tmpProjectPath, 'dist/main.js');

        await waitForPathExists(nodeScriptPath);

        const initOkFilePath = nodePath.join(tmpProjectPath, 'ok-init');

        const injectCode = encodeURIComponent(`
            import __nodeFs__ from 'node:fs';
            __nodeFs__.writeFileSync('ok-init', '');
          `.trim());

        console.log(`🔄 Starting Node.js process: ${nodeScriptPath}`);
        const runningArtifactProcess = execa(
          'node',
          ['--import', `data:text/javascript,${injectCode}`, nodeScriptPath],
          { cwd: tmpProjectPath, stdio: 'inherit' },
        );

        // Wait for the Node.js process to start
        await waitForPathExists(initOkFilePath);

        const hmrEditFiles = await collectHmrEditFiles(tmpProjectPath);

        for (const [index, [step, hmrEdits]] of hmrEditFiles.entries()) {
          console.log(
            `🔄 Processing HMR edit files for step ${step} with edits: ${
              JSON.stringify(hmrEdits, null, 2)
            }`,
          );

          // Refer to `packages/test-dev-server/src/utils/get-dev-watch-options-for-ci.ts`
          // We used a poll-based and debounced watcher in CI, so we need to wait for certain amount of time to
          // - Make sure different steps are not debounced together
          // - Make sure changes are detected individually for different steps
          // - Make sure changes in the same step are detected together
          if (index !== 0) {
            await sensibleTimeoutInMs(
              CONFIG.watch.debounceDuration + CONFIG.watch.debounceTickRate +
                100,
            );
          }

          for (const hmrEdit of hmrEdits) {
            const newContent = nodeFs.readFileSync(
              hmrEdit.replacementPath,
              'utf-8',
            );
            console.log(`🔄 Writing content to: ${hmrEdit.targetPath}`);
            nodeFs.writeFileSync(hmrEdit.targetPath, newContent);
          }
          console.log(
            `⏳ Waiting for HMR to be triggered for step ${step}`,
          );
          await waitForPathExists(
            nodePath.join(tmpProjectPath, `ok-${index}`),
            10 * 1000,
          );
          console.log(`✅ HMR triggered for step ${step}`);
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

function waitForPathExists(path: string, timeout = 6 * 1000) {
  console.log(`🔄 - Waiting for path ${path} to exist...`);
  const startTime = Date.now();
  const isTimeout = () => Date.now() - startTime > timeout;
  return new Promise<void>((resolve, reject) => {
    function check() {
      try {
        nodeFs.accessSync(path);
        console.log(`✅ - Path ${path} exists`);
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
          setTimeout(check, 100);
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
    glob.convertPathToPattern(nodePath.join(projectPath, './src')) +
      '/**/*.hmr-*.*',
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
