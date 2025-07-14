import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest';

export default defineTest({
  afterTest(output) {
    expect(output.output[0].code).toContain('console.log("foo")');
  },
})
