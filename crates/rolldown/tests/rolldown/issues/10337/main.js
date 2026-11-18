// Only `MeshBVH` is demanded from the dynamic namespace. `common_functions` must be
// reached exclusively through the facade-simulation replay (via the deferred
// `shaderIntersectFunction` body), never through this import.
import('./three-mesh-bvh/index.js').then((res) => {
  if (typeof res.MeshBVH !== 'function') {
    throw new Error('MeshBVH missing from dynamic namespace');
  }
});

import('./three-mesh-bvh/extra.js').then((res) => {
  if (res.extra !== 'extra') {
    throw new Error('extra missing from dynamic namespace');
  }
});
