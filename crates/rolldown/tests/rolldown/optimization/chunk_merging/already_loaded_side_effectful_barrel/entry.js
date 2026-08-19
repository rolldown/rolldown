import './eager-consumer.js';
import { eagerValueA } from './library/index.js';

(globalThis.fixtureLog ??= []).push(`entry:${eagerValueA()}`);

export async function loadPart(part) {
  if (part === 'a') return import('./lazy-consumer-a.js');
  if (part === 'b') return import('./lazy-consumer-b.js');
  return import('./lazy-consumer-c.js');
}
