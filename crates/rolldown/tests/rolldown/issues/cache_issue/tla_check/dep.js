export let value;

try {
  value = await Promise.resolve('dep');
} catch {}
