globalThis.loadM = () =>
  import('./m.js').then((m) => {
    m.inc();
    globalThis.events.push('Y ' + m.count);
    return m;
  });
