import { valueA } from './circular-dep-init.js';

const valueB = 'circ-dep-init-b';
const valueAB = valueA.concat(` ${valueB}`);

export function getValueAB() {
  return valueAB;
}
