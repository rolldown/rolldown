import assert from 'node:assert';

// c2 is reachable only through require edges, so it is invisible to the expected order. Its
// at-risk signal must transfer to m3, the eager module hosting its first-reach trigger, or
// m3's shared entry chunk runs c2 ahead of s during main's load. Source order under main is
// [S, C2, C1, M3, M1, A]; b then only adds B.
const logs = [];
const originalLog = console.log;
console.log = (...args) => {
  logs.push(args.join(' '));
};

try {
  await import('./dist/main.js');
  await import('./dist/b.js');
} finally {
  console.log = originalLog;
}

assert.deepStrictEqual(logs, ['S', 'C2', 'C1', 'M3', 'M1', 'A', 'B']);
