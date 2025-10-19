import { describe, expect, test } from 'vitest';
import { filterVitePlugins } from './filter-vite-plugins.js';

describe('filterVitePlugins', () => {
  test('returns empty array for null/undefined input', () => {
    expect(filterVitePlugins(null)).toEqual([]);
    expect(filterVitePlugins(undefined)).toEqual([]);
    expect(filterVitePlugins(false)).toEqual([]);
  });

  test('includes plugins without apply property', () => {
    const plugins = [
      { name: 'plugin1' },
      { name: 'plugin2' },
    ];

    const result = filterVitePlugins(plugins);
    expect(result).toEqual(plugins);
  });

  test('filters out plugins with apply: "serve"', () => {
    const plugins = [
      { name: 'plugin1', apply: 'build' },
      { name: 'plugin2', apply: 'serve' },
      { name: 'plugin3' },
    ];

    const result = filterVitePlugins(plugins);
    expect(result).toEqual([
      { name: 'plugin1', apply: 'build' },
      { name: 'plugin3' },
    ]);
  });

  test('includes plugins with apply: "build"', () => {
    const plugins = [
      { name: 'plugin1', apply: 'build' },
      { name: 'plugin2', apply: 'build' },
    ];

    const result = filterVitePlugins(plugins);
    expect(result).toEqual(plugins);
  });

  test('handles nested arrays', () => {
    const plugins = [
      { name: 'plugin1' },
      [
        { name: 'plugin2', apply: 'serve' },
        { name: 'plugin3', apply: 'build' },
      ],
      { name: 'plugin4' },
    ];

    const result = filterVitePlugins(plugins);
    expect(result).toEqual([
      { name: 'plugin1' },
      { name: 'plugin3', apply: 'build' },
      { name: 'plugin4' },
    ]);
  });

  test('handles function apply that returns true', () => {
    const plugins = [
      {
        name: 'plugin1',
        apply: () => true,
      },
    ];

    const result = filterVitePlugins(plugins);
    expect(result).toHaveLength(1);
    expect(result[0]).toHaveProperty('name', 'plugin1');
  });

  test('filters out plugins with function apply that returns false', () => {
    const plugins = [
      {
        name: 'plugin1',
        apply: () => false,
      },
      { name: 'plugin2' },
    ];

    const result = filterVitePlugins(plugins);
    expect(result).toEqual([{ name: 'plugin2' }]);
  });

  test('calls apply function with correct arguments', () => {
    let calledConfig;
    let calledEnv;

    const plugins = [
      {
        name: 'plugin1',
        apply: (config: any, env: any) => {
          calledConfig = config;
          calledEnv = env;
          return true;
        },
      },
    ];

    filterVitePlugins(plugins);

    expect(calledConfig).toEqual({});
    expect(calledEnv).toEqual({ command: 'build', mode: 'production' });
  });

  test('includes plugin if apply function throws', () => {
    const plugins = [
      {
        name: 'plugin1',
        apply: () => {
          throw new Error('test error');
        },
      },
    ];

    const result = filterVitePlugins(plugins);
    expect(result).toHaveLength(1);
    expect(result[0]).toHaveProperty('name', 'plugin1');
  });

  test('filters out falsy values in array', () => {
    const plugins = [
      { name: 'plugin1' },
      null,
      undefined,
      false,
      { name: 'plugin2' },
    ];

    const result = filterVitePlugins(plugins);
    expect(result).toEqual([
      { name: 'plugin1' },
      { name: 'plugin2' },
    ]);
  });

  test('handles single plugin (not in array)', () => {
    const plugin = { name: 'plugin1' };
    const result = filterVitePlugins(plugin);
    expect(result).toEqual([plugin]);
  });

  test('filters single plugin with apply: "serve"', () => {
    const plugin = { name: 'plugin1', apply: 'serve' };
    const result = filterVitePlugins(plugin);
    expect(result).toEqual([]);
  });

  test('complex nested scenario', () => {
    const plugins = [
      { name: 'plugin1' },
      [
        { name: 'plugin2', apply: 'serve' },
        [
          { name: 'plugin3', apply: 'build' },
          null,
          { name: 'plugin4', apply: 'serve' },
        ],
      ],
      false,
      { name: 'plugin5', apply: () => true },
      { name: 'plugin6', apply: () => false },
    ];

    const result = filterVitePlugins(plugins);
    expect(result.map((p: any) => p.name)).toEqual([
      'plugin1',
      'plugin3',
      'plugin5',
    ]);
  });
});
