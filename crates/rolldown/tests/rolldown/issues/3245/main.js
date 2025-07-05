(() => {
  const foo = () => {
    if (__BAR__) {
      console.log();
    }
  };

  /* #__PURE__ */ foo();
})();
