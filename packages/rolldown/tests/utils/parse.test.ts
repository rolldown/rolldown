import { parse, parseSync } from 'rolldown/utils';
import { expect, test } from 'vitest';

test('parse non json value', async () => {
  const result = await parse('foo.js', '1n');
  expect(result.program.body[0]).toMatchInlineSnapshot(`
    {
      "end": 2,
      "expression": {
        "bigint": "1",
        "end": 2,
        "raw": "1n",
        "start": 0,
        "type": "Literal",
        "value": 1n,
      },
      "start": 0,
      "type": "ExpressionStatement",
    }
  `);
});

test('parseSync non json value', () => {
  const result = parseSync('foo.js', '1n');
  expect(result.program.body[0]).toMatchInlineSnapshot(`
    {
      "end": 2,
      "expression": {
        "bigint": "1",
        "end": 2,
        "raw": "1n",
        "start": 0,
        "type": "Literal",
        "value": 1n,
      },
      "start": 0,
      "type": "ExpressionStatement",
    }
  `);
});
