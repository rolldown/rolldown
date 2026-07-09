import assert from 'node:assert/strict';

import { rolldown } from 'rolldown';
import { configureAsyncRuntime, getAsyncRuntimeConfig, scan } from 'rolldown/experimental';

configureAsyncRuntime({
  flavor: 'MultiThread',
  workerThreads: 2,
  maxBlockingTasks: 1,
});
const configBeforeInvalidOptions = getAsyncRuntimeConfig();
const invalidOptions = [
  ['workerThreads', 0],
  ['workerThreads', 1.5],
  ['workerThreads', Number.NaN],
  ['workerThreads', Number.POSITIVE_INFINITY],
  ['workerThreads', 2 ** 32],
  ['maxBlockingTasks', 0],
  ['maxBlockingTasks', 1.5],
  ['maxBlockingTasks', Number.NaN],
  ['maxBlockingTasks', Number.POSITIVE_INFINITY],
  ['maxBlockingTasks', 2 ** 32],
];
for (const [field, value] of invalidOptions) {
  assert.throws(
    () =>
      configureAsyncRuntime({
        flavor: 'CurrentThread',
        workerThreads: field === 'workerThreads' ? value : 1,
        maxBlockingTasks: field === 'maxBlockingTasks' ? value : 1,
      }),
    (error) =>
      error instanceof Error &&
      error.message.includes(`\`${field}\` must be a positive integer no greater than 4294967295`),
  );
  assert.deepEqual(
    getAsyncRuntimeConfig(),
    configBeforeInvalidOptions,
    `rejected ${field}=${String(value)} must not partially mutate the runtime configuration`,
  );
}

configureAsyncRuntime({
  flavor: 'CurrentThread',
  workerThreads: 1,
  maxBlockingTasks: 1,
});

const createInputOptions = () => ({
  input: 'virtual:main',
  plugins: [
    {
      name: 'async-runtime-flavor-switch',
      resolveId(id) {
        if (id === 'virtual:main') return `\0${id}`;
      },
      load(id) {
        if (id === '\0virtual:main') {
          return 'export const answer = 42;';
        }
      },
    },
  ],
});

await scan(createInputOptions());

const bundle = await rolldown(createInputOptions());
let buildSettled = false;
try {
  const output = await bundle.generate({ format: 'esm' });
  buildSettled = output.output.length > 0;
} finally {
  await bundle.close();
}

console.log(
  JSON.stringify({
    flavor: getAsyncRuntimeConfig().flavor,
    invalidConfigurationsRejected: invalidOptions.length,
    scanSettled: true,
    buildSettled,
  }),
);
