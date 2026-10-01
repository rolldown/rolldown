const HEAVY_MARKER = 'heavy-marker';
// Written as one literal on purpose. A top-level `[...].join(':')` would be a module
// side effect rolldown cannot prove away, and a side-effectful family *must* execute
// whenever the barrel executes — which puts it in every consumer's closure by design.
const HEAVY_PAYLOAD =
  'heavy-payload-01:heavy-payload-02:heavy-payload-03:heavy-payload-04:' +
  'heavy-payload-05:heavy-payload-06:heavy-payload-07:heavy-payload-08';

export function heavyValue() {
  return `${HEAVY_MARKER}:${HEAVY_PAYLOAD}`;
}
