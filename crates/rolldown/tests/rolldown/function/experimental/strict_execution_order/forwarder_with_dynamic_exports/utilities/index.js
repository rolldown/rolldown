import { isEqual } from '../shared-equality.js';

export { isEqual };

export function hasExpectedValue(actual) {
  return actual === 'expected';
}

export * from 'node:path';
