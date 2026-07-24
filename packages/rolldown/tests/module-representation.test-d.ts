import type { ModuleInfo, RepresentType, SourceDescription } from 'rolldown';
import { expectTypeOf, test } from 'vitest';

test('exposes representation metadata types', () => {
  const values = [
    'js',
    'json',
    'text',
    'base64',
    'dataurl',
    'binary',
    'empty',
    'url',
    'copy',
  ] as const satisfies readonly RepresentType[];
  expectTypeOf<(typeof values)[number]>().toEqualTypeOf<RepresentType>();

  const source = { code: 'export default 1', representType: 'url' } satisfies SourceDescription;
  expectTypeOf(source.representType).toEqualTypeOf<'url'>();

  const info = undefined as unknown as ModuleInfo;
  expectTypeOf(info.representType).toEqualTypeOf<RepresentType | undefined>();

  // @ts-expect-error unknown representation types are rejected
  const invalid: RepresentType = 'asset';
  void invalid;
});
