import { bump, count, marker } from './shared.js';
globalThis.events.push('B ' + count + ' ' + bump?.() + ' ' + count);
globalThis.markers.push(marker);
