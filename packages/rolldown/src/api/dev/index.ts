import { DevEngine } from './dev-engine';

export const dev: typeof DevEngine.create = (...args) =>
  DevEngine.create(...args);
