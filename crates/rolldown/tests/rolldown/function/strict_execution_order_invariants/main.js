export { value as mainValue } from './shared.js';

export function loadDynamic() {
  return import('./dynamic.js');
}
