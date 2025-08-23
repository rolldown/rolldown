import type { Foo } from './foo';

class Bar implements Foo {
  a: string = 'a';
}

console.log(new Bar());
