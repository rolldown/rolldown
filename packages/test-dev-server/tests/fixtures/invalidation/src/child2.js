import * as mod from './child2';

if (import.meta.hot) {
  import.meta.hot.accept((_newExports) => {
    globalThis.records.push(Object.keys(mod));
    import.meta.hot.invalidate();
  });
}

export const value2 = 'child2';
