import { parse as pa } from './parser-a.js';
import { parse as pb } from './parser-b.js';
import { parse as pc } from './parser-c.js';
async function opt() {
  await import('@optional/ext');
}
export function transform(code) {
  opt();
  return pa(code) + pb(code) + pc(code);
}
