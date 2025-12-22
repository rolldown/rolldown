import { replacePlugin } from 'rolldown/plugins';
import { expect, test } from 'vitest';

test('error on invalid delimiters', () => {
  expect(() => {
    replacePlugin(
      {
        'process.env.NODE_ENV': JSON.stringify('production'),
      },
      {
        delimiters: ['(', ''],
      },
    );
  }).toThrowError('Unbalanced parenthesis');
});
