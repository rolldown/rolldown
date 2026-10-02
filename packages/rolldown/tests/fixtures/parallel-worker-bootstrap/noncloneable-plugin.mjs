import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';

export default defineParallelPluginImplementation(() => {
  const thrownValue = function nonCloneableBootstrapFailure() {};
  thrownValue.message = 'non-cloneable parallel bootstrap failure';
  throw thrownValue;
});
