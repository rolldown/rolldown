export let foo;
try {
  foo = await Promise.resolve('foo');
} catch (_e) {}