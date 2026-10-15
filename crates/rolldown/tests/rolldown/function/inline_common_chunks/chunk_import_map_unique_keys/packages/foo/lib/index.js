import { label } from '../../../common/shared.js';
export const foo = label('foo');
export function fooDescribe(value) {
  return 'foo received ' + value + ' and produced ' + foo;
}
export function fooRepeat(times) {
  return Array.from({ length: times }, () => foo).join(',');
}
