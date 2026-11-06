import * as self from './dep.js';

export const obj = {};
const returned = {};
export const fn = () => returned;
export class C {
  #x = 0;
  static poke() {
    self.inst.#x = 1;
    return self.inst.#x;
  }
}
export const inst = new C();
