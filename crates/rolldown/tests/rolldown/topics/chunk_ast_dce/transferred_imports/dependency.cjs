if (globalThis.__transferFirst !== 17 || globalThis.__transferSecond !== 25)
  throw new Error('transferred init order');
globalThis.__transferEvents.push('dependency');
