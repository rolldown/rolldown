import { count as vendorCount } from './vendor.js';
import { bump, count, marker } from './shared.js';
// The entry's own bindings reuse every name the shared module declares or imports.
var helper = 'entry-helper';
var init_shared = 'entry-init';
function count$1() {
  return 'entry-count$1';
}
globalThis.events.push(
  'A ' +
    bump() +
    ' ' +
    count +
    ' ' +
    helper +
    ' ' +
    init_shared +
    ' ' +
    count$1() +
    ' ' +
    vendorCount,
);
globalThis.markers.push(marker);
