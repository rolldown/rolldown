import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import {
  checkPrepareVersion,
  compareVersions,
  parseReleaseLine,
  parseReleaseSubject,
  resolvePublish,
} from './release-line.mjs';

describe('parseReleaseLine', () => {
  test('main is the current line', () => {
    assert.deepEqual(parseReleaseLine('main'), { kind: 'main' });
  });

  test('vX.Y is a maintenance line', () => {
    assert.deepEqual(parseReleaseLine('v1.2'), { kind: 'maintenance', major: 1, minor: 2 });
    assert.deepEqual(parseReleaseLine('v12.30'), { kind: 'maintenance', major: 12, minor: 30 });
  });

  test('rejects other branches', () => {
    for (const branch of ['v1', 'v1.2.3', 'release/1.2', 'vite-tests', 'feat/x']) {
      assert.throws(() => parseReleaseLine(branch), /main or a vX\.Y maintenance branch/);
    }
  });
});

describe('compareVersions', () => {
  test('orders by major, minor and patch numerically', () => {
    assert.ok(compareVersions('1.2.10', '1.2.9') > 0);
    assert.ok(compareVersions('1.10.0', '1.9.9') > 0);
    assert.ok(compareVersions('2.0.0', '1.99.99') > 0);
    assert.equal(compareVersions('1.2.3', '1.2.3'), 0);
  });

  test('a prerelease is older than its release', () => {
    assert.ok(compareVersions('1.3.0-beta.1', '1.3.0') < 0);
    assert.ok(compareVersions('1.3.0', '1.3.0-rc.9') > 0);
  });

  test('orders prerelease identifiers', () => {
    assert.ok(compareVersions('1.3.0-beta.2', '1.3.0-beta.10') < 0);
    assert.ok(compareVersions('1.3.0-alpha.9', '1.3.0-beta.0') < 0);
    assert.ok(compareVersions('1.3.0-beta', '1.3.0-beta.0') < 0);
    assert.ok(compareVersions('1.3.0-1', '1.3.0-alpha') < 0);
  });

  test('rejects non-semver input', () => {
    assert.throws(() => compareVersions('1.2', '1.2.0'), /semver/);
    assert.throws(() => compareVersions('v1.2.0', '1.2.0'), /semver/);
  });
});

describe('checkPrepareVersion', () => {
  test('main accepts any newer version', () => {
    checkPrepareVersion({ branch: 'main', version: '1.2.17', currentVersion: '1.2.16' });
    checkPrepareVersion({ branch: 'main', version: '1.3.0', currentVersion: '1.2.16' });
    checkPrepareVersion({ branch: 'main', version: '1.3.0-beta.0', currentVersion: '1.2.16' });
  });

  test('main rejects a version that is not newer', () => {
    assert.throws(
      () => checkPrepareVersion({ branch: 'main', version: '1.2.16', currentVersion: '1.2.16' }),
      /must be greater/,
    );
    assert.throws(
      () => checkPrepareVersion({ branch: 'main', version: '1.1.6', currentVersion: '1.2.16' }),
      /must be greater/,
    );
  });

  test('a maintenance branch accepts the next patch of its line', () => {
    checkPrepareVersion({ branch: 'v1.1', version: '1.1.6', currentVersion: '1.1.5' });
  });

  test('a maintenance branch rejects other lines and prereleases', () => {
    assert.throws(
      () => checkPrepareVersion({ branch: 'v1.1', version: '1.2.0', currentVersion: '1.1.5' }),
      /v1\.1 only releases 1\.1\.x/,
    );
    assert.throws(
      () =>
        checkPrepareVersion({ branch: 'v1.1', version: '1.1.6-beta.0', currentVersion: '1.1.5' }),
      /stable patches/,
    );
  });

  test('rejects branches that are not release lines', () => {
    assert.throws(
      () =>
        checkPrepareVersion({ branch: 'release/1.1', version: '1.1.6', currentVersion: '1.1.5' }),
      /maintenance branch/,
    );
  });
});

describe('parseReleaseSubject', () => {
  test('reads the version from release commits', () => {
    assert.equal(parseReleaseSubject('release: v1.2.17'), '1.2.17');
    assert.equal(parseReleaseSubject('release: v1.2.16 (#11383)'), '1.2.16');
    assert.equal(parseReleaseSubject('release: v1.0.0-rc.17 (#9001)'), '1.0.0-rc.17');
  });

  test('ignores other commits', () => {
    for (const subject of [
      'fix: release: v1.2.17',
      'chore(release): v1.2.17',
      'release: perf-kit@0.1.0',
      'release: v1.2.17 extra',
      'Release: v1.2.17',
    ]) {
      assert.equal(parseReleaseSubject(subject), undefined);
    }
  });
});

describe('resolvePublish', () => {
  test('skips pushes that are not release commits', () => {
    assert.deepEqual(
      resolvePublish({
        branch: 'main',
        subject: 'fix: something (#1)',
        version: '1.2.16',
        npmLatest: undefined,
      }),
      { release: false },
    );
  });

  test('main publishes under latest and marks the GitHub release as latest', () => {
    assert.deepEqual(
      resolvePublish({
        branch: 'main',
        subject: 'release: v1.2.17 (#11400)',
        version: '1.2.17',
        npmLatest: '1.2.16',
      }),
      { release: true, version: '1.2.17', distTag: 'latest', makeLatest: true },
    );
  });

  test('main allows rerunning a release that already reached npm', () => {
    assert.equal(
      resolvePublish({
        branch: 'main',
        subject: 'release: v1.2.17 (#11400)',
        version: '1.2.17',
        npmLatest: '1.2.17',
      }).release,
      true,
    );
  });

  test('main refuses to move latest back', () => {
    assert.throws(
      () =>
        resolvePublish({
          branch: 'main',
          subject: 'release: v1.2.17',
          version: '1.2.17',
          npmLatest: '1.3.0',
        }),
      /older than npm latest/,
    );
  });

  test('a maintenance branch publishes under its own dist-tag', () => {
    assert.deepEqual(
      resolvePublish({
        branch: 'v1.1',
        subject: 'release: v1.1.6 (#11401)',
        version: '1.1.6',
        npmLatest: '1.2.16',
      }),
      { release: true, version: '1.1.6', distTag: 'latest-1.1', makeLatest: false },
    );
  });

  test('a maintenance branch refuses versions that are not older than latest', () => {
    assert.throws(
      () =>
        resolvePublish({
          branch: 'v1.2',
          subject: 'release: v1.2.17',
          version: '1.2.17',
          npmLatest: '1.2.16',
        }),
      /not older than npm latest 1\.2\.16; release it from main/,
    );
    assert.throws(
      () =>
        resolvePublish({
          branch: 'v1.1',
          subject: 'release: v1.1.6',
          version: '1.1.6',
          npmLatest: undefined,
        }),
      /not older than npm latest \(none\)/,
    );
  });

  test('a maintenance branch refuses versions from another line', () => {
    assert.throws(
      () =>
        resolvePublish({
          branch: 'v1.1',
          subject: 'release: v1.0.4',
          version: '1.0.4',
          npmLatest: '1.2.16',
        }),
      /v1\.1 only releases 1\.1\.x/,
    );
  });

  test('refuses a release commit that does not match package.json', () => {
    assert.throws(
      () =>
        resolvePublish({
          branch: 'main',
          subject: 'release: v1.2.18',
          version: '1.2.17',
          npmLatest: '1.2.16',
        }),
      /release commit is for v1\.2\.18/,
    );
  });

  test('refuses release commits on other branches', () => {
    assert.throws(
      () =>
        resolvePublish({
          branch: 'feat/x',
          subject: 'release: v1.2.17',
          version: '1.2.17',
          npmLatest: '1.2.16',
        }),
      /maintenance branch/,
    );
  });
});
