// Other half of the two-chunk static import cycle (paired with cyc-a.js).
import './cyc-a.js';

(globalThis.__events ??= []).push('cyc-b');

export const b = 1;
