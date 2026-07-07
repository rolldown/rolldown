import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';
import { Worker } from 'node:worker_threads';

const bindingDir = fileURLToPath(new URL('../../../src/', import.meta.url));
const bindingFiles = readdirSync(bindingDir).filter(
  (name) => name.startsWith('rolldown-binding.') && name.endsWith('.node'),
);
if (bindingFiles.length !== 1) {
  throw new Error(`Expected one native Rolldown binding, found ${bindingFiles.join(', ')}`);
}

const directory = mkdtempSync(nodePath.join(tmpdir(), 'rolldown-waker-teardown-'));
const paths = {
  armed: nodePath.join(directory, 'armed'),
  completed: nodePath.join(directory, 'completed'),
  release: nodePath.join(directory, 'release'),
};

try {
  const worker = new Worker(new URL('./worker.mjs', import.meta.url), {
    workerData: {
      bindingPath: nodePath.join(bindingDir, bindingFiles[0]),
      paths,
    },
  });
  const started = await new Promise((resolve, reject) => {
    worker.once('message', resolve);
    worker.once('error', reject);
  });
  if (started.type !== 'started') {
    throw new Error(started.error ?? `Unexpected worker response: ${JSON.stringify(started)}`);
  }

  await waitForFile(paths.armed, paths.completed, 'scheduler waker publication');
  await worker.terminate();
  const workerExitedBeforeRelease = true;

  writeFileSync(paths.release, 'release');
  await waitForFile(paths.completed, undefined, 'post-teardown scheduler waker completion');
  const completed = readFileSync(paths.completed, 'utf8');
  if (completed !== 'completed') {
    throw new Error(completed);
  }

  console.log(
    JSON.stringify({
      backend: started.backend,
      completed,
      workerExitedBeforeRelease,
    }),
  );
} finally {
  rmSync(directory, { force: true, recursive: true });
}

async function waitForFile(path, failurePath, label) {
  const deadline = Date.now() + 10_000;
  while (!existsSync(path)) {
    if (failurePath && existsSync(failurePath)) {
      throw new Error(readFileSync(failurePath, 'utf8'));
    }
    if (Date.now() >= deadline) {
      throw new Error(`Timed out waiting for ${label}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}
