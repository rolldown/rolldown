import { parentPort } from 'node:worker_threads';
import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';

export default defineParallelPluginImplementation(({ disruptReporting }) => {
  if (disruptReporting) {
    parentPort.postMessage = () => {
      throw new Error('forced bootstrap postMessage failure');
    };
  }

  const thrownValue = function nonCloneableBootstrapFailure() {};
  thrownValue.message = 'non-cloneable parallel bootstrap failure';
  throw thrownValue;
});
