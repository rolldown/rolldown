import { Foo } from 'ext';
import { make } from './b.js';
console.log(Foo, make().constructor.name);
