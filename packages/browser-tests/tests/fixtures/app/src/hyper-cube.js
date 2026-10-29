import cube from './cube.js';

// Only imported by one entry, so it shares that entry's chunk
export default function hyperCube(x) {
  return cube(x) * x;
}
