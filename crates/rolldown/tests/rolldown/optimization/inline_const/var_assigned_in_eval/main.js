export const before = flag;
export var flag = true;
export var other = true;

export function change() {
  eval('flag = false; other = false');
}

export function read() {
  return [flag, other];
}
