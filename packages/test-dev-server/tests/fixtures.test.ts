import { execa, ExecaError, execaSync } from 'execa';
import glob from 'fast-glob';
import killPort from 'kill-port';
import nodeFs from 'node:fs';
import nodeFsPromise from 'node:fs/promises';
import nodeOs from 'node:os';
import nodePath from 'node:path';
import { rimrafSync } from 'rimraf';
import { afterAll, test } from 'vitest';

function removeDirSync(path: string) {
  if (nodeOs.platform() === 'win32') {
    // 1. Seems any nodejs-based solution to remove a directory resulted to EBUSY error on Windows
    // 2. Check if the path exists before trying to remove it, otherwise it will throw an error
    execaSync(
      `if exist "${path}" rmdir /s /q "${path}"`,
      {
        shell: true,
        stdio: 'inherit',
      },
    );
  } else {
    rimrafSync(path);
  }
}

function main() {
  const tmpFixturesPath = nodePath.resolve(__dirname, 'tmp/fixtures');

  removeDirSync(tmpFixturesPath);

  afterAll(() => {
    removeDirSync(tmpFixturesPath);
  });

  test('basic', async () => {
    const projectName = 'basic';
    const tmpProjectPath = nodePath.join(
      tmpFixturesPath,
      projectName,
    );
    copyProjectToTmp(projectName);
    await execa('pnpm install --no-frozen-lockfile', {
      cwd: tmpProjectPath,
      shell: true,
      stdio: 'inherit',
    });

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

    const runningArtifactProcess = execa(
      `node ${nodeScriptPath}`,
      { cwd: tmpProjectPath, shell: true, stdio: 'inherit' },
    );

    await new Promise<void>((rsl, _rej) => {
      setTimeout(rsl, 4000);
    });

    const hmrEditFiles = await collectHmrEditFiles(tmpProjectPath);

    for (const hmrEditFile of hmrEditFiles) {
      const newContent = await nodeFsPromise.readFile(
        hmrEditFile.path,
        'utf-8',
      );
      await nodeFsPromise.writeFile(hmrEditFile.targetPath, newContent);
      console.debug(
        `Waiting for HMR to be triggered... ${hmrEditFile.targetPath}`,
      );
      await ensurePathExists(nodePath.join(tmpProjectPath, 'ok-1'));
      console.debug(`Successfully triggered HMR ${hmrEditFile.targetPath}`);
    }

    try {
      runningArtifactProcess.kill('SIGTERM');
      await runningArtifactProcess;
    } catch (err) {
      if (err instanceof ExecaError && err.signal === 'SIGTERM') {
        console.log('Process killed normally with SIGTERM, ignoring error.');
      } else {
        throw err;
      }
    }
    try {
      devServeProcess.kill('SIGTERM');
      await devServeProcess;
    } catch (err: any) {
      if (err instanceof ExecaError && err.signal === 'SIGTERM') {
        console.log('Process killed normally with SIGTERM, ignoring error.');
      } else {
        throw err;
      }
    }
  });
}

function copyProjectToTmp(projectName: string) {
  const projectPath = nodePath.resolve(__dirname, `fixtures/${projectName}`);
  const tmpProjectPath = nodePath.resolve(
    __dirname,
    `tmp/fixtures/${projectName}`,
  );

  // Copy project to temp directory. Remember to filter out `node_modules` and `dist` directories
  nodeFs.mkdirSync(tmpProjectPath, { recursive: true });
  nodeFs.cpSync(projectPath, tmpProjectPath, {
    recursive: true,
    filter: (src) => {
      return !src.includes('node_modules') && !src.includes('dist');
    },
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
