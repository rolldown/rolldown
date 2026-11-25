import { logged, log } from './lib.js';
@logged
export class Foo {
  @logged x = 1;
  @logged accessor y = 2;
  @logged greet(n) {
    return `hi ${n} ${this.x}`;
  }
  static #secret = 7;
  @logged static reveal() {
    return Foo.#secret;
  }
}
export default
@logged
class {}
export { log };
