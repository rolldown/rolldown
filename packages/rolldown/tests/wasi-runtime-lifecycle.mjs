import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const childFlag = '--run-wasi-runtime-lifecycle-suite';

if (process.argv.includes(childFlag)) {
  await import('./wasi-runtime-lifecycle-case.mjs');
} else {
  await runWithWatchdog();
}

async function runWithWatchdog() {
  const timeoutMilliseconds = Number(process.env.ROLLDOWN_WASI_LIFECYCLE_TIMEOUT_MS ?? 10 * 60_000);
  if (!Number.isFinite(timeoutMilliseconds) || timeoutMilliseconds <= 0) {
    throw new Error('ROLLDOWN_WASI_LIFECYCLE_TIMEOUT_MS must be a positive number');
  }

  const child = spawn(process.execPath, [fileURLToPath(import.meta.url), childFlag], {
    stdio: 'inherit',
  });
  let timedOut = false;
  let forceKillTimer;
  const timeout = setTimeout(() => {
    timedOut = true;
    child.kill('SIGTERM');
    forceKillTimer = setTimeout(() => child.kill('SIGKILL'), 5_000);
  }, timeoutMilliseconds);

  const { code, signal } = await new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('exit', (code, signal) => resolve({ code, signal }));
  }).finally(() => {
    clearTimeout(timeout);
    clearTimeout(forceKillTimer);
  });

  if (timedOut) {
    throw new Error(`threaded-WASI lifecycle suite timed out after ${timeoutMilliseconds}ms`);
  }
  if (code !== 0) {
    throw new Error(
      `threaded-WASI lifecycle suite exited with ${signal ? `signal ${signal}` : `code ${code}`}`,
    );
  }
}
