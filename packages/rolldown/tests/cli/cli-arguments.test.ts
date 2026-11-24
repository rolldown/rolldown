import { expect, test } from 'vitest';
import { parseCliArguments } from '../../src/cli/arguments/index';

test.each([
  { args: ['--', 'entry.js'], input: ['entry.js'] },
  { args: ['--', 'first.js', 'second.js'], input: ['first.js', 'second.js'] },
  { args: ['first.js', '--', 'second.js'], input: ['first.js', 'second.js'] },
  { args: ['--', '-entry.js', '--help'], input: ['-entry.js', '--help'] },
  { args: ['first.js', '--'], input: ['first.js'] },
])('preserves positional arguments after --: $args', ({ args, input }) => {
  const originalArgv = process.argv;
  try {
    process.argv = ['node', 'rolldown', ...args];
    const options = parseCliArguments();
    expect(options.input.input).toEqual(input);
    expect(options.help).toBe(false);
    expect(options.rawArgs).not.toHaveProperty('--');
  } finally {
    process.argv = originalArgv;
  }
});

test('option names cannot pollute Object.prototype', () => {
  const originalArgv = process.argv;
  try {
    process.argv = [
      'node',
      'rolldown',
      'entry.js',
      '--__proto__.polluted=1',
      '--constructor.prototype.polluted=1',
      '--checks.__proto__.polluted=1',
    ];
    parseCliArguments();
    expect(({} as Record<string, unknown>).polluted).toBeUndefined();
  } finally {
    process.argv = originalArgv;
  }
});
