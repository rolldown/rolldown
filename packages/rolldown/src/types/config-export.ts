import type { RolldownOptions } from './rolldown-options';
import type { RolldownOptionsFunction } from './rolldown-options-function';

/**
 * Type for `default export` of `rolldown.config.js` file.
 */
export type ConfigExport =
  | RolldownOptions
  | RolldownOptions[]
  | RolldownOptionsFunction;
