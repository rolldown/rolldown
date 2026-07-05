import { moduleRunnerTransform as originalModuleRunnerTransform } from '../binding.cjs';
import { leaseAsyncFunction } from './run-with-runtime-lease';

export const moduleRunnerTransform: typeof originalModuleRunnerTransform = leaseAsyncFunction(
  originalModuleRunnerTransform,
  'Module runner transform and runtime release both failed',
);
