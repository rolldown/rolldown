import square from './square.js';

// Shared by both entries, so it lands in a common chunk
export default function cube(x) {
  return square(x) * x;
}
