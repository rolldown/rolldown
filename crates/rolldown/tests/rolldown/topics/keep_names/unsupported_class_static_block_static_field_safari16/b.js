import { Baz as B } from './c.js';

// Declares `#_`, so the `__name` field takes `#_2`.
export class Qux {
  static #_ = 'qux';
  static tag = this.name;
  static read() {
    return Qux.#_;
  }
}

// The inner class reads `Holder.#_`, so its `__name` field must not be named `#_`.
export class Holder {
  static #_ = 'holder';
  static make() {
    return class Baz {
      static tag = this.name;
      static read() {
        return Holder.#_;
      }
      static base() {
        return B;
      }
    };
  }
}
