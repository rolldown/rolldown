import { DevEngine } from './dev-engine';

export var dev: typeof DevEngine.create = DevEngine.create;

export type { DevOptions, DevWatchOptions } from './dev-options';
