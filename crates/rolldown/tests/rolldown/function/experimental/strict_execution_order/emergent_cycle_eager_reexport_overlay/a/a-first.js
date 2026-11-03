// Earliest-discovered side-effectful member of chunk A: makes the entry load chunk A before chunk
// B. This pins the chunk direction around the carrier while the forwarder's retained obligation
// tests whether lowering introduces an unnecessary reverse edge.
(globalThis.__events ??= []).push('a-first');
export const aFirst = true;
