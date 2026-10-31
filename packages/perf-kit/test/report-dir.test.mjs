// The report dir rule must match `report_dir_for` in
// crates/rolldown_devtools_metrics/src/lib.rs, or the lab reads a stale or missing report.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';

import { moduleGraphCandidates, reportDirFor } from '../lib/module-graph.mjs';

const cwd = path.resolve('/work/app');
const report = (...segments) => path.join(cwd, 'node_modules', '.rolldown', 'report', ...segments);

test('one report dir per output dir', () => {
  assert.equal(reportDirFor(cwd, 'dist'), report('dist'));
  assert.equal(reportDirFor(cwd, 'dist/client'), report('dist', 'client'));
  assert.equal(reportDirFor(cwd, './dist/'), report('dist'));
  assert.equal(reportDirFor(cwd, path.join(cwd, 'dist', 'server')), report('dist', 'server'));
  assert.equal(reportDirFor(cwd, '.'), report());
  assert.equal(reportDirFor(cwd, ''), report());
});

test('an output dir outside cwd writes each `..` as `__`', () => {
  assert.equal(reportDirFor(cwd, '../shared/dist'), report('__', 'shared', 'dist'));
});

test('module graph candidates walk up from the dist dir', () => {
  const dist = path.join(cwd, 'build', 'dist');
  assert.deepEqual(moduleGraphCandidates({ dist }), [
    path.join(cwd, 'build', 'node_modules', '.rolldown', 'report', 'dist', 'module-graph.json'),
    path.join(report('build', 'dist'), 'module-graph.json'),
    path.join(
      cwd,
      '..',
      'node_modules',
      '.rolldown',
      'report',
      'app',
      'build',
      'dist',
      'module-graph.json',
    ),
  ]);
});
