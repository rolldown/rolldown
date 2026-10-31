import { parseModuleId, stringifyModuleId } from 'rolldown/utils';
import { expect, test } from 'vitest';

test('stringifyModuleId', () => {
  expect(stringifyModuleId('/src/a.js', {})).toBe('/src/a.js');
  expect(stringifyModuleId('/src/a.json', { type: 'json' })).toBe(
    '/src/a.json?attributes=%7B%22type%22%3A%22json%22%7D',
  );
  // Key order does not matter.
  expect(stringifyModuleId('/src/logo.png', { type: 'image', size: '200' })).toBe(
    stringifyModuleId('/src/logo.png', { size: '200', type: 'image' }),
  );
  expect(stringifyModuleId('/src/icon.svg?raw', { type: 'text' })).toBe(
    '/src/icon.svg?raw&attributes=%7B%22type%22%3A%22text%22%7D',
  );
});

test('parseModuleId', () => {
  expect(parseModuleId('/src/a.json?attributes=%7B%22type%22%3A%22json%22%7D')).toEqual({
    rawId: '/src/a.json',
    attributes: { type: 'json' },
  });
  expect(parseModuleId('/src/icon.svg?raw&attributes=%7B%22type%22%3A%22text%22%7D')).toEqual({
    rawId: '/src/icon.svg?raw',
    attributes: { type: 'text' },
  });
  for (const id of [
    '/src/a.js?raw',
    'virtual?attributes=foo',
    'a?attributes=%7B%7D',
    'a?attributes=%7B%22type%22%3A1%7D',
    'a?attributes=%7b%22type%22%3a%22json%22%7d',
  ]) {
    expect(parseModuleId(id)).toEqual({ rawId: id, attributes: {} });
  }
});
