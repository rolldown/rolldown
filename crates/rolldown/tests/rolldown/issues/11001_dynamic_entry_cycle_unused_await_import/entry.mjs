import { setMarker } from './state.mjs';
await import('./a.mjs');
setMarker(2);
const { value } = await import('./z.mjs');
console.log(value);
