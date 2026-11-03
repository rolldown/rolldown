import { default as Foo } from './foo.cjs';

const Bar = Foo.Bar;

export { Foo, Bar };
export { default as Other } from './other.cjs';
