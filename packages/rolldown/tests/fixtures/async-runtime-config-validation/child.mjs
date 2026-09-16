import { configureAsyncRuntime, getAsyncRuntimeConfig } from 'rolldown/experimental';

const rejectionMessage = (options) => {
  try {
    configureAsyncRuntime(options);
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  throw new Error(`Expected configureAsyncRuntime to reject ${JSON.stringify(options)}`);
};

const workerErrors = [-1, 0, 1.5, Number.NaN, Number.POSITIVE_INFINITY, 4_294_967_296].map(
  (workerThreads) => rejectionMessage({ workerThreads }),
);
const blockingError = rejectionMessage({ maxBlockingTasks: -1 });

configureAsyncRuntime({ workerThreads: 2, maxBlockingTasks: 1 });

console.log(
  JSON.stringify({
    blockingError,
    config: getAsyncRuntimeConfig(),
    workerErrors,
  }),
);
