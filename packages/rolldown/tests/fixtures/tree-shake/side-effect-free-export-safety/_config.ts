import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      // This fixture checks tree-shaker metadata before final chunk DCE reparses the output.
      minify: false,
    },
    treeshake: true,
  },
  afterTest: (output) => {
    const code = output.output
      .filter((chunk): chunk is OutputChunk => chunk.type === 'chunk')
      .map((chunk) => chunk.code)
      .join('\n');

    expect(code).toContain('reassignedSideEffect');
    expect(code).toContain('reassignedResult = reassigned();');
    expect(code).toContain('var maybeFn = function() {};');
    expect(code).toContain('conditionalBinding = maybeFn;');
    expect(code).toContain('evalReassigned = () =>');
    expect(code).toContain('evalReassigned();');
    expect(code).toContain('cyclic();');
    expect(code).toContain('defaultCyclic();');
    expect(code).toContain('defaultCycleAfterCall');
    expect(code).toContain('localEarlyAccess();');
    expect(code).toContain('barrelFn();');
    expect(code).toContain('restDefaultSideEffect');
    expect(code).toContain('restDefault();');
    expect(code).not.toContain('/* @__PURE__ */ reassigned()');
    expect(code).not.toContain('/* @__PURE__ */ maybeFn()');
    expect(code).not.toContain('/* @__PURE__ */ evalReassigned()');
    expect(code).not.toContain('/* @__PURE__ */ cyclic()');
    expect(code).not.toContain('/* @__PURE__ */ defaultCyclic()');
    expect(code).not.toContain('/* @__PURE__ */ localEarlyAccess()');
    expect(code).not.toContain('/* @__PURE__ */ barrelFn()');
    expect(code).not.toContain('/* @__PURE__ */ restDefault()');
    expect(code).not.toContain('unusedDefaultMarker');
  },
});
