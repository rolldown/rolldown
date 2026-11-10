import defer * as route from './route.js';

globalThis.__deferPlanEvents = [];

export function run() {
  const a = route.read(false);
  const before = [...globalThis.__deferPlanEvents];
  const b = route.read(true);
  return { a, before, b, after: globalThis.__deferPlanEvents };
}
