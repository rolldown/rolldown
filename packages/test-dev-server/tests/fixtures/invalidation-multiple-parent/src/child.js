if (import.meta.hot) {
  import.meta.hot.accept((_newExports) => {
    import.meta.hot.invalidate();
  });
}

export const value = 'child';
