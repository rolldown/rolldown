import getLogFilter, { type RollupLog } from 'rolldown/getLogFilter';
import { describe, expect, it } from 'vitest';

describe('getLogFilter', () => {
  it('does not filter when there are no filters', () => {
    const filter = getLogFilter([]);
    expect(filter({ code: 'FIRST' } as RollupLog)).toBe(true);
  });

  it('filters for string matches', () => {
    const filter = getLogFilter(['code:FIRST']);
    expect(filter({ code: 'FIRST', message: '' } as RollupLog)).toBe(true);
    expect(filter({ code: 'SECOND', message: '' } as RollupLog)).toBe(false);
    expect(filter({ message: 'no code' } as RollupLog)).toBe(false);
  });

  it('combines multiple filters with "or"', () => {
    const filter = getLogFilter(['code:FIRST', 'message:second']);
    expect(filter({ code: 'FIRST', message: 'first' })).toBe(true);
    expect(filter({ code: 'SECOND', message: 'first' })).toBe(false);
    expect(filter({ code: 'FIRST', message: 'second' })).toBe(true);
    expect(filter({ code: 'SECOND', message: 'second' })).toBe(true);
  });

  it('supports placeholders', () => {
    const filter = getLogFilter(['code:*A', 'code:B*', 'code:*C*', 'code:D*E*F']);
    expect(filter({ code: 'xxA', message: '' }), 'xxA').toBe(true);
    expect(filter({ code: 'xxB', message: '' }), 'xxB').toBe(false);
    expect(filter({ code: 'Axx', message: '' }), 'Axx').toBe(false);
    expect(filter({ code: 'Bxx', message: '' }), 'Bxx').toBe(true);
    expect(filter({ code: 'C', message: '' }), 'C').toBe(true);
    expect(filter({ code: 'xxCxx', message: '' }), 'xxCxx').toBe(true);
    expect(filter({ code: 'DxxExxF', message: '' }), 'DxxExxF').toBe(true);
  });

  it('supports inverted filters', () => {
    const filter = getLogFilter(['!code:FIRST']);
    expect(filter({ code: 'FIRST', message: '' })).toBe(false);
    expect(filter({ code: 'SECOND', message: '' })).toBe(true);
  });

  it('supports AND conditions', () => {
    const filter = getLogFilter(['code:FIRST&plugin:my-plugin']);
    expect(filter({ code: 'FIRST', plugin: 'my-plugin', message: '' })).toBe(true);
    expect(filter({ code: 'FIRST', plugin: 'other-plugin', message: '' })).toBe(false);
    expect(filter({ code: 'SECOND', plugin: 'my-plugin', message: '' })).toBe(false);
  });

  it('handles numbers and objects', () => {
    const filter = getLogFilter(['foo:1', 'bar:*2*', 'baz:{"a":1}', 'baz:{"b":1,*}']);
    expect(filter({ foo: 1, message: '' } as any), 'foo:1').toBe(true);
    expect(filter({ foo: 10, message: '' } as any), 'foo:10').toBe(false);
    expect(filter({ bar: 123, message: '' } as any), 'bar:123').toBe(true);
    expect(filter({ bar: 13, message: '' } as any), 'bar:13').toBe(false);
    expect(filter({ baz: { a: 1 }, message: '' } as any), 'baz:{"a":1}').toBe(true);
    expect(filter({ baz: { a: 1, b: 2 }, message: '' } as any), 'baz:{"a":1,"b":2}').toBe(false);
    expect(filter({ baz: { b: 1, c: 2 }, message: '' } as any), 'baz:{"b":1,"c":2}').toBe(true);
  });

  it('handles edge case filters', () => {
    const filter = getLogFilter([
      ':A', // property is "empty string"
      'a:', // value is "empty string"
      '', // property and value are "empty string"
      'code:A&', // property and value are "empty string",
      'foo:bar:baz', // second colon is treated literally
    ]);
    expect(filter({ '': 'A', message: '' } as any), ':A').toBe(true);
    expect(filter({ foo: 'A', message: '' } as any), 'foo:A').toBe(false);
    expect(filter({ a: '', message: '' } as any), 'a:').toBe(true);
    expect(filter({ a: 'foo', message: '' } as any), 'a:foo').toBe(false);
    expect(filter({ '': '', message: '' } as any), '').toBe(true);
    expect(filter({ code: 'A', message: '' } as any), 'code:A').toBe(false);
    expect(filter({ code: 'A', '': '', message: '' } as any), 'code:A&').toBe(true);
    expect(filter({ foo: 'bar:baz', message: '' } as any), 'foo:bar:baz').toBe(true);
  });

  it('handles nested properties', () => {
    const filter = getLogFilter(['foo.bar:baz']);
    expect(filter({ foo: null, message: '' } as any), 'foo:bar').toBe(false);
    expect(filter({ foo: { bar: 'baz' }, message: '' } as any), 'foo.bar:baz').toBe(true);
    expect(filter({ foo: { bar: 'qux' }, message: '' } as any), 'foo.bar:qux').toBe(false);
    expect(filter({ foo: { bar: { baz: 'qux' } }, message: '' } as any), 'foo.bar.baz:qux').toBe(
      false,
    );
  });
});
