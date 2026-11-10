import defer * as root from './root.js';

function read() {
  try {
    return root.value;
  } catch (error) {
    return error;
  }
}

export function run() {
  const first = read();
  const second = read();
  return { first, second, runs: globalThis.__memberRuns };
}
