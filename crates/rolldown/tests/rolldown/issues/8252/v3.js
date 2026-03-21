import { resolveJsFrom } from './resolve';

export function loadV3() {
  return resolveJsFrom('v3-dir', 'v3-config');
}
