export function f() {
  return this === undefined;
}
export function tag() {
  return this === undefined;
}
export class K {
  constructor() {
    this.ok = true;
  }
}
export const obj = {
  m() {
    return this === obj;
  },
};
