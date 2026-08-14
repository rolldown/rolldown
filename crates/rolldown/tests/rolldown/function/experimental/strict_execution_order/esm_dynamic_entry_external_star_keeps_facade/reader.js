import { x } from './target.js';

export function read() {
  globalThis.__externalStarLog.push(`reader:${x}`);
}
