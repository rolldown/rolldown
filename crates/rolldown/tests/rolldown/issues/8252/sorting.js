import { resolveJsFrom } from './resolve';

export function sort(input) {
  return resolveJsFrom(input, 'some-pkg');
}
