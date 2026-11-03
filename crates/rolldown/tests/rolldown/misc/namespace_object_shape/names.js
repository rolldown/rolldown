// These export names are also the names of `Object.prototype` members.
const proto = 'proto export';

export function constructor() {
  return 'constructor export';
}

export function toString() {
  return 'toString export';
}

export function valueOf() {
  return 'valueOf export';
}

export { proto as __proto__ };
