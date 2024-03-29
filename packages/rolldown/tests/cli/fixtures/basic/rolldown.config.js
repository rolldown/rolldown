export default {
  input: 'src/index.js',
  output: [
    {
      dir: 'build',
      file: 'build/bundle.js',
    },
  ],
  resolve: {
    conditionNames: ['import'],
    alias: {
      modules: 'src/modules',
    },
  },
}
