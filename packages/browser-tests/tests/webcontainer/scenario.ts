import { test } from '@webcontainer/test';
import { expect } from 'vitest';
import {
  type Container,
  expectFreshArtifact,
  expectNoPreexistingDist,
  expectNoRegistryFallback,
  run,
} from './utils';

// the app under test, shared by every scenario; only the dependencies differ
const APP = 'tests/fixtures/app';

export interface Scenario {
  /** fixture directory holding this scenario's package.json and packed tarballs */
  overlay: string;
  /** what is under test, shown in the test name */
  subject: string;
  /** build command run inside the container */
  build: string;
  /** scenario-specific checks on the installed tree, between install and build */
  verifyInstall?: (container: Container) => Promise<void>;
}

// Each scenario stays in its own test file so vitest runs them in parallel browser instances,
// which roughly halves wall time compared to two tests in one file.
export function defineScenario(scenario: Scenario) {
  test(`the ${scenario.subject} builds the shared app inside WebContainer`, async ({
    webcontainer,
  }) => {
    // CI runs the runner's system Chrome, which drifts with the runner image, so a failing
    // run has to record which browser it ran on
    console.log(navigator.userAgent);

    // mount() overlays rather than replaces, so the scenario's overlay lands on the shared app
    await webcontainer.mount(APP);
    await webcontainer.mount(scenario.overlay);
    await expectNoPreexistingDist(webcontainer);

    const install = await run(webcontainer, 'pnpm install --no-frozen-lockfile');
    expect(install, install.output).toMatchObject({ exitCode: 0 });

    await expectFreshArtifact(webcontainer);
    await scenario.verifyInstall?.(webcontainer);

    const build = await run(webcontainer, scenario.build);
    expect(build, build.output).toMatchObject({ exitCode: 0 });
    expectNoRegistryFallback(build.output);

    // two entries plus one shared chunk for the code both entries reach
    const dist = (await webcontainer.readdir('/dist')).sort();
    expect(dist).toHaveLength(3);
    expect(dist).toContain('entry.js');
    expect(dist).toContain('other-entry.js');
    const sharedChunk = dist.find((name) => /^cube-.*\.js$/.test(name));
    expect(sharedChunk, `no shared chunk in ${dist.join(', ')}`).toBeTruthy();

    const entry = await webcontainer.readFile('/dist/entry.js');
    expect(entry).toContain('console.log(hyperCube(5))');
    expect(entry).toContain(`from "./${sharedChunk}"`);

    const otherEntry = await webcontainer.readFile('/dist/other-entry.js');
    expect(otherEntry).toContain('console.log(cube(5))');
    expect(otherEntry).toContain(`from "./${sharedChunk}"`);

    const shared = await webcontainer.readFile(`/dist/${sharedChunk}`);
    expect(shared).toContain('function square');
    expect(shared).toContain('function cube');
  });
}
