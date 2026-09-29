import collision from './collision.js';
import parenthesized from './parenthesized.cjs';
import paths from './paths.cjs';

const expectedPaths = [import.meta.dirname, import.meta.filename];

if (JSON.stringify(paths) !== JSON.stringify(expectedPaths)) {
  throw new Error(`Unexpected direct eval paths: ${JSON.stringify(paths)}`);
}

if (JSON.stringify(parenthesized) !== JSON.stringify(expectedPaths)) {
  throw new Error(`Unexpected parenthesized eval paths: ${JSON.stringify(parenthesized)}`);
}

const expectedCollision = [...expectedPaths, 'imported dirname', 'imported filename'];
if (JSON.stringify(collision) !== JSON.stringify(expectedCollision)) {
  throw new Error(`Unexpected collision values: ${JSON.stringify(collision)}`);
}
