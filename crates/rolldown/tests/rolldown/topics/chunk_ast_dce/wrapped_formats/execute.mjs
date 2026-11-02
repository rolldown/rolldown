import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

const { code, format, entry, extend } = JSON.parse(readFileSync(0, 'utf8'));
const branches = format === 'umd' ? ['browser', 'cjs', 'amd'] : ['browser'];
const marker = entry === 'none' ? 'no-exports-wrapper-anchor' : `${entry}-wrapper-anchor`;
const expected = {
  value: 42,
  offset: 3,
  sameDefault: true,
  privateValues: ['first', 'second', 'nested'],
};
const plain = (value) => JSON.parse(JSON.stringify(value));

for (const branch of branches) {
  const events = [];
  const external = Object.assign(
    (value) => {
      events.push('external-call');
      return value + 2;
    },
    { offset: 3 },
  );
  const sandbox = { __wrappedEvents: events, __wrappedInput: 40 };
  const existing = { retained: 17 };
  const parent = { sibling: 23, api: existing };
  if (branch === 'browser') {
    let accesses = 0;
    sandbox.Dependencies = {
      get wrapped() {
        accesses++;
        events.push('global-external');
        return external;
      },
    };
    if (extend) sandbox.Wrapped = { nested: parent, other: 29 };
    runInNewContext(code, sandbox);
    assert.equal(accesses, 1, 'browser external evaluated once');
  } else if (branch === 'cjs') {
    const exports = { retained: 17 };
    sandbox.exports = exports;
    sandbox.module = { exports };
    sandbox.require = (id) => {
      assert.equal(id, 'wrapped-external');
      events.push('require-external');
      return external;
    };
    sandbox.define = () => assert.fail('CJS takes precedence over AMD');
    sandbox.define.amd = {};
    runInNewContext(code, sandbox);
  } else {
    const exports = { retained: 17 };
    let definitions = 0;
    sandbox.define = (dependencies, factory) => {
      definitions++;
      assert.deepEqual(
        Array.from(dependencies),
        entry === 'named' ? ['exports', 'wrapped-external'] : ['wrapped-external'],
      );
      events.push('amd-external');
      const result = factory(
        ...Array.from(dependencies, (id) => (id === 'exports' ? exports : external)),
      );
      sandbox.amdResult = entry === 'named' ? exports : result;
    };
    sandbox.define.amd = {};
    runInNewContext(code, sandbox);
    assert.equal(definitions, 1, 'one AMD registration');
  }

  const acquisition = { browser: 'global-external', cjs: 'require-external', amd: 'amd-external' }[
    branch
  ];
  assert.deepEqual(events.slice(0, 3), [acquisition, 'private-wrapper-anchor', marker]);
  let exported;
  if (entry !== 'none') {
    exported =
      branch === 'browser'
        ? extend
          ? sandbox.Wrapped.nested.api
          : sandbox.Wrapped
        : branch === 'cjs'
          ? sandbox.module.exports
          : sandbox.amdResult;
  }
  if (entry === 'named') {
    assert.equal(exported.current, 1);
    exported.increment();
    assert.equal(exported.current, 2, 'named export stays live');
    assert.deepEqual(plain(exported.read()), expected);
    if (branch === 'cjs' || branch === 'amd') assert.equal(exported.retained, 17);
  } else if (entry === 'default') {
    assert.equal(typeof exported, 'function', 'default export is the factory result');
    assert.deepEqual(plain(exported()), expected);
  } else {
    assert.deepEqual(plain(sandbox.__wrappedResult), expected);
    assert.equal(Object.hasOwn(sandbox, 'Wrapped'), false);
    if (branch === 'cjs') assert.deepEqual(sandbox.module.exports, { retained: 17 });
    if (branch === 'amd') assert.equal(sandbox.amdResult, undefined);
  }
  assert.deepEqual(events, [acquisition, 'private-wrapper-anchor', marker, 'external-call']);
  if (extend && branch === 'browser') {
    assert.equal(exported, existing, 'extend preserves existing exports identity');
    assert.equal(sandbox.Wrapped.nested, parent, 'dotted parent stays intact');
    assert.equal(exported.retained, 17);
    assert.equal(sandbox.Wrapped.nested.sibling, 23);
    assert.equal(sandbox.Wrapped.other, 29);
  }
}
