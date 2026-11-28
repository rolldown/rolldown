import { qv } from './q.js';
console.log('eval p');
// Reads q.js while p.js evaluates: q.js must run first.
export const P = 'P' + qv;
