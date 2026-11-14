import { NAMES } from './names.js';
import { formatLabel } from './label.js';

(globalThis.__execOrder ??= []).push('heavy');

export function Heavy() {
  return NAMES.map((n, i) => `${formatLabel(i)}:${n}`).join(',');
}
