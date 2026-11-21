import './b.js';

export { annotated };

try {
  later();
} catch (error) {
  globalThis.calls.push(`same module: ${error.name}`);
}

const later = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('later');
const annotated = /* @__NO_SIDE_EFFECTS__ */ () =>
  globalThis.calls.push('removed: an export clause does not read the binding');
annotated();
