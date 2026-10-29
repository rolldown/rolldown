// Earliest side-effectful member of chunk A: makes the entry import chunk A before chunk B.
(globalThis.__events ??= []).push('a-first');
export const aFirst = true;
