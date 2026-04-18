import assert from 'node:assert';
import { eagerValue, loadRouteComponent } from './route-graph.js';

assert.strictEqual(eagerValue, 'shared-eager');

loadRouteComponent().then((mod) => {
  assert.strictEqual(mod.componentValue, 'shared-lazy');
});
