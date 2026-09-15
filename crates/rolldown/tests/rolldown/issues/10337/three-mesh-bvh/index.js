export * from './core/MeshBVH.js';
export * as BVHShaderGLSL from './webgl/BVHShaderGLSL.js';

// Mirrors three-mesh-bvh's "backwards compatibility" block: a top-level statement that
// reads through the namespace re-export chain. Under `moduleSideEffects: false` this
// body is deferred at link time (only `MeshBVH` is demanded by the dynamic import) and
// resurrected by the facade-chunk-elimination replay.
import * as BVHShaderGLSL from './webgl/BVHShaderGLSL.js';
export const shaderIntersectFunction = `
	${BVHShaderGLSL.common_functions}
`;
