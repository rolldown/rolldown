// Init-owning barrel (own top-level effect). Its excluded star hop serves BOTH a resolved member
// read (records a retained path for definer1's chain) and the opaque namespace (which retains
// every export, including definer2's chain that no resolved read recorded).
export * from './inner.js';

(globalThis.__events ??= []).push('barrel');
