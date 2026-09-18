import * as ns from './cjs.js';
globalThis.events.push('B ' + ns.default.bump() + ' ' + ns.bump());
globalThis.markers.push(ns.default);
