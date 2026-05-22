import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const fn = vi.fn();

export default defineTest({
  config: {
    onLog(_level: any, log: any, defaultHandler: any) {
      fn();
      // The message should NOT contain ariadne's "Warning:" prefix
      expect(log.message).not.toMatch(/^Warning:/);
      defaultHandler('error', log);
    },
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalled();
  },
  catchError(err: any) {
    expect(err).toBeInstanceOf(Error);
  },
});
