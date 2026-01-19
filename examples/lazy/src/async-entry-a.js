import asyncLibA from './async-lib-a.js';
import './async-lib-shared.js';

console.log('async-entry-a.js', asyncLibA);
document.getElementById('root').innerHTML += '[async-entry-a.js] loaded\n';

class Foo {
  // eslint-disable-next-line no-unused-private-class-members
  static #_ = (this.foo = 0);
}
console.log(new Foo().foo);
