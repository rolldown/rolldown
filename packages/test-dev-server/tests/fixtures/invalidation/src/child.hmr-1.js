if (import.meta.hot) {
  import.meta.hot.accept((newExports) => {
    if (newExports.value === 'child-edited') {
      import.meta.hot.invalidate();
    }
  });
}

export const value = 'child-edited';
