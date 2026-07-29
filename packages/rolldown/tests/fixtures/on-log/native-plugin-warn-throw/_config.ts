import { defineTest } from 'rolldown-tests';
import { esmExternalRequirePlugin } from 'rolldown/plugins';
import { expect, vi } from 'vitest';

const fn = vi.fn();

export default defineTest({
  config: {
    external: ['ext'],
    plugins: [esmExternalRequirePlugin({ external: ['ext'] })],
    onLog(_level, log) {
      fn();
      expect(log.message).toContain('duplicate external');
      throw new Error('convert log to error');
    },
  },
  afterTest: () => {
    // The native plugin warning must actually have reached `onLog`...
    expect(fn).toHaveBeenCalled();
    // ...and the build should then have aborted. Reaching here means the error
    // thrown from `onLog` was swallowed (native_plugin_context.rs).
    throw new Error(
      'Expected the build to fail because the onLog handler threw, but it succeeded.',
    );
  },
  catchError(err: any) {
    expect(fn).toHaveBeenCalled();
    expect(err).toBeInstanceOf(Error);
    expect(err.message).toContain('convert log to error');
  },
});
