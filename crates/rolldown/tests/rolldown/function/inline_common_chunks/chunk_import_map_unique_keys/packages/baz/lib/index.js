import { label } from '../../../common/shared.js';
export const baz = label('baz');
export function bazDescribe(value) {
  return 'baz received ' + value + ' and produced ' + baz;
}
export function bazRepeat(times) {
  return Array.from({ length: times }, () => baz).join(',');
}
