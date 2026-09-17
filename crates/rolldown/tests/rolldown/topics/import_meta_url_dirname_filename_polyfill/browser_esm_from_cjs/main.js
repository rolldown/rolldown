import probe from './probe.cjs';

const expected = {
  ambient: ['undefined', 'undefined'],
  locals: ['local dirname', 'local filename'],
};

if (JSON.stringify(probe) !== JSON.stringify(expected)) {
  throw new Error(`Unexpected CommonJS path bindings: ${JSON.stringify(probe)}`);
}
