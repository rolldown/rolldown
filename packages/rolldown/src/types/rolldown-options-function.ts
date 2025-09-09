import type { RolldownOptions } from './rolldown-options';
import type { MaybePromise } from './utils';

export type RolldownOptionsFunction = (
  commandLineArguments: Record<string, any>,
) => MaybePromise<RolldownOptions | RolldownOptions[]>;
