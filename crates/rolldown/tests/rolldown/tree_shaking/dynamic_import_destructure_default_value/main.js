import('./lib.js').then((ns) => {
  const { used = 1 } = ns;
  const { unused: u2 } = ns; // force two destructures
  console.log(used, u2);
});
