export function setX() {
  obj.x = 1;
}
try {
  setX();
} catch {}
const obj = { x: 0 };
export function check() {
  return obj.x === 1 ? 'changed' : 'same';
}
