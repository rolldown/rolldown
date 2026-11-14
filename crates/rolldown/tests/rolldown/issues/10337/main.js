// Only `MeshBVH` is demanded from the dynamic namespace. `common_functions` must be
// reached exclusively through the facade-simulation replay (via the deferred
// `shaderIntersectFunction` body), never through this import.
import('./three-mesh-bvh/index.js').then((res) => {
  if (typeof res.MeshBVH !== 'function') {
    throw new Error('MeshBVH missing from dynamic namespace');
  }
});
