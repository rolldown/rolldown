import external from './external.cjs';
globalThis.__cjsEvents.push('cjs-entry-anchor');
export let current = 1;
export function increment() {
  current++;
}
const result = external.value + 2;
export { result as default };
