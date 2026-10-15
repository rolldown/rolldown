import { label } from '../../../common/shared.js';
export const bar = label('bar');
export function barDescribe(value) {
  return 'bar received ' + value + ' and produced ' + bar;
}
export function barRepeat(times) {
  return Array.from({ length: times }, () => bar).join(',');
}
