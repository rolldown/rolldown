if (globalThis.__configName === 'known-failure') {
  console.error('known variant execution failure');
  process.exitCode = 1;
}
