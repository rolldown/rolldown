import { bump, count, marker } from './shared.js';
const helper = () => 'entry-b-helper';
globalThis.events.push('B ' + bump() + ' ' + count + ' ' + helper());
globalThis.markers.push(marker);
