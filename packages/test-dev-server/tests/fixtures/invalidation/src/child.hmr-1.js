import nodeFs from 'node:fs';

if (import.meta.hot) {
  import.meta.hot.accept((newExports) => {
    if (newExports.value === 'child-handleable') {
      globalThis.records.push('child-handleable');
      nodeFs.writeFileSync('./ok-0', '');
    } else if (newExports.value === 'child-unhandleable') {
      globalThis.records.push('child-unhandleable');
      import.meta.hot.invalidate();
    }
  });
}

export const value = 'child-handleable';
