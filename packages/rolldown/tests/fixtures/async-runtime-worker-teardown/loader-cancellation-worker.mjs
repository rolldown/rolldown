import { parentPort } from 'node:worker_threads';

import { rolldown } from 'rolldown';

const entryPublicId = 'virtual:worker-termination';
const entryResolvedId = `\0${entryPublicId}`;
const normalPublicId = 'virtual:pending-normal';
const normalResolvedId = `\0${normalPublicId}`;
const externalId = 'external:pending';
const bundle = await rolldown({
  input: entryPublicId,
  plugins: [
    {
      name: 'pending-worker-load',
      resolveId(id) {
        if (id === entryPublicId) {
          return entryResolvedId;
        }
        if (id === normalPublicId) {
          return normalResolvedId;
        }
        if (id === externalId) {
          return { id, external: true };
        }
      },
      load(id) {
        if (id === entryResolvedId) {
          return [
            `import ${JSON.stringify(normalPublicId)};`,
            `import ${JSON.stringify(externalId)};`,
            'export const value = 42;',
          ].join('\n');
        }
        if (id === normalResolvedId) {
          parentPort.postMessage('normal-load-entered');
          return new Promise(() => {});
        }
      },
    },
  ],
  treeshake: {
    moduleSideEffects(id, external) {
      if (id === externalId && external) {
        parentPort.postMessage('external-side-effects-entered');
        return new Promise(() => {});
      }
      return true;
    },
  },
});

await bundle.generate({ format: 'esm' });
