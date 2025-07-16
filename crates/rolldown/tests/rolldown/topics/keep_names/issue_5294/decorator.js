import bar from './main.js';
import { strictEqual } from 'node:assert';

export default function foo() {
  strictEqual(bar.name, 'foo')
  bar();
}

strictEqual(foo.name, 'foo');
