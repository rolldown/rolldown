// Side-effectful module hosted in the entry chunk. Source order runs it before the manager, but
// the manager sits in a grouped chunk the entry chunk imports, so the predicted evaluation order
// runs the manager's chunk first — a real order deviation that puts the manager (and, through the
// plan closure, the facade and the entry) into the on-demand wrap plan without a chunk cycle.
(globalThis.__events ??= []).push('first');

export const ready = true;
