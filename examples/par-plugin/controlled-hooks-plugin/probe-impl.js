import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';

export const createProbePlugin = (options, threadNumber = 0) => {
  let instanceCalls = 0;
  const statePerWorkerCalls = options.stateBuffer ? new Int32Array(options.stateBuffer) : undefined;

  switch (options.mode) {
    case 'filter-miss':
      return {
        name: 'controlled-filter-miss-probe',
        resolveId: {
          filter: { id: { include: [/^this-never-matches:/] } },
          handler() {
            throw new Error('native filter called the excluded handler');
          },
        },
      };
    case 'state':
      return {
        name: 'controlled-instance-state-probe',
        resolveId: {
          filter: { id: { include: [/^controlled-state:/] } },
          handler(specifier) {
            instanceCalls++;
            if (statePerWorkerCalls) Atomics.add(statePerWorkerCalls, threadNumber, 1);
            const index = Number(specifier.slice('controlled-state:'.length));
            return `\0controlled-state:${instanceCalls}:${index}`;
          },
        },
      };
    case 'reentrant':
      return {
        name: 'controlled-reentrant-probe',
        resolveId: {
          filter: { id: { include: [/^controlled-reentrant:/] } },
          async handler(specifier, importer) {
            if (specifier === 'controlled-reentrant:inner') {
              return '\0controlled-reentrant-result';
            }
            if (specifier !== 'controlled-reentrant:outer') {
              throw new Error(`unexpected reentrant probe specifier: ${specifier}`);
            }
            return this.resolve('controlled-reentrant:inner', importer, { skipSelf: false });
          },
        },
      };
    case 'resolve-error':
      return {
        name: 'controlled-resolve-error-probe',
        resolveId: {
          filter: { id: { include: [/^controlled-resolve-error:/] } },
          handler() {
            throw new Error('controlled resolveId error');
          },
        },
      };
    case 'load-error':
      return {
        name: 'controlled-load-error-probe',
        load: {
          filter: { id: { include: [/^\0controlled-load-error:/] } },
          async handler() {
            throw new Error('controlled load error');
          },
        },
      };
    default:
      throw new Error(`invalid correctness probe mode: ${options.mode}`);
  }
};

export default defineParallelPluginImplementation((options, context) =>
  createProbePlugin(options, context.threadNumber),
);
