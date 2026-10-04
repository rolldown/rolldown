const hasOwnProperty = Object.prototype.hasOwnProperty;

export function isEqual(left, right) {
  return left === right || (typeof left === 'object' && hasOwnProperty.call(left, right));
}
