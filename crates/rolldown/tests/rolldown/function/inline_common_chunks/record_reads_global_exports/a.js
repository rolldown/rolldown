import { kind, n } from './shared.js';
// A local with the name the bridge would get; reassigned so it is not inlined as a constant.
let share_shared = 'entry-a';
share_shared += '';
globalThis.events.push('A ' + kind + ' ' + n + ' ' + share_shared);
