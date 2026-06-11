// Child process for dev-engine-close.test.ts: prove that `DevEngine.close()`
// releases everything keeping the process alive (watcher threads, napi
// threadsafe callbacks) and that a second engine can start in the same
// process after the first closed. The parent asserts that this script exits
// on its own — a leak would make it hang until the parent's timeout.
import nodePath from 'node:path';
import { dev } from 'rolldown/experimental';

const fixtureDir = nodePath.resolve(import.meta.dirname, '../fixtures/edit');

for (let round = 0; round < 2; round++) {
  const engine = await dev(
    { cwd: fixtureDir, input: ['./src/main.js'], treeshake: false },
    {},
    {
      onOutput: () => {},
      watch: { skipWrite: true },
    },
  );
  await engine.run();
  await engine.close();
  console.log(`dev-engine-close-child: round ${round} closed`);
}

console.log('dev-engine-close-child: OK');
