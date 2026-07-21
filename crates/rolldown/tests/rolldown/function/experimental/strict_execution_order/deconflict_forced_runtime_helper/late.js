// Ordered after `helper`, so its wrapper's `init_late = __esmMin(...)` runs after `helper` clobbered
// the shared `__esmMin` — the call that throws on the unfixed build.
export const late = 'LATE:' + (globalThis.__y ?? 'z');
