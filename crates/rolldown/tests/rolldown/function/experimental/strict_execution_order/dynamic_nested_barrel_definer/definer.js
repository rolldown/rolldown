(globalThis.__events ??= []).push('definer');

// A runtime-computed value (not a bare literal) so it cannot be constant-inlined away: the reader
// must actually read it through the barrel chain, which is what forces the barrel wrappers to
// initialize this definer.
function makeValue() {
  return 5;
}
export const value = makeValue();
