import { transform } from './transform'

export default {
  input: 'src/index.ts',
  plugins: [
    {
      name: 'test-plugin',
      transform,
    },
  ],
}
