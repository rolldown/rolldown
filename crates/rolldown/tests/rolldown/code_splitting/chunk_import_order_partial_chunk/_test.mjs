import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
for (const [entry, expected] of [
  ['b', 'ready:marker'],
  ['c', 'unset:marker'],
]) {
  const url = new URL('./dist/' + entry + '.js', import.meta.url).href;
  const code = `const ns = await import(${JSON.stringify(url)}); console.log(ns.read());`;
  const value = execFileSync(process.execPath, ['--input-type=module', '-e', code], {
    encoding: 'utf8',
  }).trim();
  assert.equal(value, expected);
}
