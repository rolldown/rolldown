if (
  typeof process !== 'undefined' && typeof process.env === 'object' &&
  process.env.NODE_ENV === 'production'
) {
  console.log('production');
}
