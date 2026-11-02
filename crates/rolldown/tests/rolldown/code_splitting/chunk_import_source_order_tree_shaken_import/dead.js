import { data } from './data.js';

globalThis.dead = true;

export function unused() {
  return data;
}
