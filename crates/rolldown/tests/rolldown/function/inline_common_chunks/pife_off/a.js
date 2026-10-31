import { bump, count, marker } from './shared.js';
globalThis.events.push('A ' + count + ' ' + bump() + ' ' + count);
globalThis.markers.push(marker);
