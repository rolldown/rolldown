import { Checkbox, Radio } from './barrel.js';
import './cyc-a.js';

(globalThis.__events ??= []).push('reader');

// The imported components are read only later, from this deferred "render" function (like a React
// component body referencing a `@carbon/react` component). By then the barrel's `init_*` must have
// forwarded to each component's `init_*`; the regression dropped that forward, so the components
// were still `undefined` at render time.
export function render() {
  const c = Checkbox ? Checkbox.name : 'NO_CHECKBOX';
  const r = Radio ? Radio.name : 'NO_RADIO';
  return `${c}|${r}`;
}
