export var e = 'b';
export const bLog = [];
try {
  throw 'caught-b';
} catch (e) {
  var e = 'b2';
  bLog.push(e);
}
e += '!';
bLog.push(e);
export function getB() {
  return e;
}
