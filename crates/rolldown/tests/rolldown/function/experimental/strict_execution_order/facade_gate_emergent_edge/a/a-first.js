// Earliest side-effectful member of chunk A, so the root imports chunk A first.
(globalThis.__events ??= []).push('a-first');
export const aFirst = true;
