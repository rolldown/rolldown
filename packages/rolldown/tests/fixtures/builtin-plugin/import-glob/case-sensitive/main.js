const caseSensitiveModules = import.meta.glob('./dir/data-*.js', {
  eager: true,
  import: 'default',
});

const caseInsensitiveModules = import.meta.glob('./dir/data-*.js', {
  eager: true,
  import: 'default',
  caseSensitive: false,
});

const caseInsensitiveNegatedModules = import.meta.glob(
  ['./dir/data-*.js', '!./dir/DATA-other.js'],
  {
    eager: true,
    import: 'default',
    caseSensitive: false,
  },
);

export { caseSensitiveModules, caseInsensitiveModules, caseInsensitiveNegatedModules };
