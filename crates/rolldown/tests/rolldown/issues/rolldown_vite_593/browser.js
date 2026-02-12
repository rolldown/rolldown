import { compareVersions } from './shared.js';

export function isIE() {
  return compareVersions('11', '10') >= 0;
}

export function isEdge() {
  return compareVersions('12', '10') >= 0;
}
