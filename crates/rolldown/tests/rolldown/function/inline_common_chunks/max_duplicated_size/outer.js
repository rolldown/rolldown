import { inner } from './inner.js';

// Padding keeps this module's source well above the inner one, so that its three extra copies
// cross `maxDuplicatedSize` on their own.
export function outer() {
  const label = ['outer', 'reads', inner()].join(' ');
  return label.length > 0 ? label : 'unreachable fallback that only exists to add source bytes';
}
