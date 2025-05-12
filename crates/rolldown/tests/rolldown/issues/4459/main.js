import { camelCase } from 'es-toolkit';

export * from 'es-toolkit';

export function test(str) {
  return camelCase(str);
}
