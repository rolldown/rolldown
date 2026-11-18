export const modules = import.meta.glob(['../docs/**/*.js', '!**/skip-*.js', '!**/en/**'], {
  eager: true,
  import: 'default',
});
