// User module whose top-level `__esmMin` collides with the forced runtime helper. Reading a global
// keeps the value materialized (not const-folded) so its order wrapper hoists a root-scope
// `var __esmMin`; the initializer then reassigns that shared binding to the user string.
export const __esmMin = globalThis.__x ?? 'USERVAL';
export const helper = 'H:' + __esmMin;
