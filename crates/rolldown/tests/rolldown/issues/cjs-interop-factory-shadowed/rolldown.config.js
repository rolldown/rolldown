module.exports = [
  {
    input: ['entry1.js'],
    output: {
      format: 'cjs',
      entryFileNames: '[name]-pass1.js',
      chunkFileNames: '[name]-pass1.js',
    },
  },
  {
    input: ['entry2.js'],
    output: {
      format: 'cjs',
      entryFileNames: '[name]-pass2.js',
      chunkFileNames: '[name]-pass2.js',
    },
  },
];
