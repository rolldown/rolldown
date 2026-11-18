import { Baz as B } from './b.js';

export class Foo {
  static tag = this.name;
  static tag2 = Foo.name;
}

// `B` is printed as `Baz`, so the class gets its own name renamed.
export const Named = class Baz {
  static tag = this.name;
  static base() {
    return B;
  }
};
