// A module-level linked catch: `e` is a top-level binding of the chunk.
export var e = 'a';
export const aLog = [];
try {
  throw 'caught-a';
} catch (e) {
  aLog.push(e);
  var e = 'a2';
  aLog.push(e);
}
aLog.push(e);
export function getA() {
  return e;
}
