import { describe, expect, test } from 'vitest';
import { ModuleInfoAccess } from '../../src/plugin/module-info-access';

describe('ModuleInfoAccess', () => {
  test('a hook call that does nothing has nothing to report', () => {
    const access = new ModuleInfoAccess();
    expect(access.collect()).toBeUndefined();
    expect(access.isOpen).toBe(false);
  });

  test('the first use names its reason and its module', () => {
    const meta = new ModuleInfoAccess();
    meta.useMeta('/a.js');
    meta.assignModuleSideEffects('/b.js');
    meta.readModuleSideEffects('/c.js');
    expect(meta.collect()).toEqual({ reason: 'meta', moduleKey: '/a.js' });

    const assigned = new ModuleInfoAccess();
    assigned.assignModuleSideEffects('/b.js');
    assigned.useMeta('/a.js');
    expect(assigned.collect()).toEqual({ reason: 'moduleSideEffectsAssigned', moduleKey: '/b.js' });

    const read = new ModuleInfoAccess();
    read.readModuleSideEffects('/c.js');
    read.readModuleSideEffects('/d.js');
    expect(read.collect()).toEqual({ reason: 'moduleSideEffectsRead', moduleKey: '/c.js' });
  });

  test('a use after the hook call ended does not count', () => {
    const access = new ModuleInfoAccess();
    expect(access.collect()).toBeUndefined();
    access.useMeta('/a.js');
    access.assignModuleSideEffects('/a.js');
    access.readModuleSideEffects('/a.js');
    expect(access.collect()).toBeUndefined();

    const closed = new ModuleInfoAccess();
    closed.close();
    closed.useMeta('/a.js');
    expect(closed.collect()).toBeUndefined();
  });
});
