import { parse, parseSync, Visitor } from 'rolldown/utils';
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

test('Visitor is supported', () => {
  const result = parseSync('foo.js', 'function greet() { return 1; }');
  const order: string[] = [];
  const visitor = new Visitor({
    FunctionDeclaration() {
      order.push('enter');
    },
    'FunctionDeclaration:exit'() {
      order.push('exit');
    },
  });
  visitor.visit(result.program);
  expect(order).toEqual(['enter', 'exit']);
});
