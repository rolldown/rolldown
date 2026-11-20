try {
  EARLY;
} catch (error) {
  globalThis.calls.push(`define: ${error.name}`);
}
try {
  null ?? coalesced();
} catch (error) {
  globalThis.calls.push(`coalesce: ${error.name}`);
}

const early = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('early');
const coalesced = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('coalesced');
const late = /* @__NO_SIDE_EFFECTS__ */ () =>
  globalThis.calls.push('removed: LATE runs after the declaration');
LATE;
const last = /* @__NO_SIDE_EFFECTS__ */ () => {};
