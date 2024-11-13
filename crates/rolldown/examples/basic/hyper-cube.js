// import cube from './cube.js';

// This is only imported by one entry module and
// shares a chunk with that module
export default function hyperCube(x) {
	return cube(x) * x;
}

export function test() {
  return 100;
}

export function a() {
  return 100;
}
