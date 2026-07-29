import { MessagePort, workerData } from 'node:worker_threads';
import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';

export default defineParallelPluginImplementation(({ disruptReporting }) => {
  if (disruptReporting) {
    if (workerData.controlPort !== undefined) {
      workerData.controlPort.postMessage({ type: 'success' });
      throw new Error('parallel bootstrap control port leaked into plugin code');
    }
    MessagePort.prototype.postMessage = () => {
      throw new Error('forced bootstrap postMessage failure');
    };
  }

  const thrownValue = function nonCloneableBootstrapFailure() {};
  thrownValue.message = 'non-cloneable parallel bootstrap failure';
  throw thrownValue;
});
