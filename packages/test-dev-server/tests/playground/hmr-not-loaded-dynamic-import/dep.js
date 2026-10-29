// This file is never loaded.
if (import.meta.hot) {
  import.meta.hot.accept(() => {});
}

export const value = 'dep';
