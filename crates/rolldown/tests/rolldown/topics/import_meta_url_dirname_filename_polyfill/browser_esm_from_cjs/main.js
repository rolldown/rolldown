import paths from './paths.cjs';

if (JSON.stringify(paths) !== JSON.stringify(['undefined', 'undefined'])) {
  throw new Error(`Unexpected CommonJS path bindings: ${JSON.stringify(paths)}`);
}
