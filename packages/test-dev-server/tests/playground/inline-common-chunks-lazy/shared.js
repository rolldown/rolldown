import { leaf } from './leaf.js';

globalThis.__sharedRuns = (globalThis.__sharedRuns ?? 0) + 1;

export function shared(name) {
  return `${name}:${leaf()}`;
}
