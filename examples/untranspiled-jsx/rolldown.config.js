import { defineConfig } from 'rolldown';

export default defineConfig({
  input: './main.jsx',
  transform: {
    jsx: 'preserve',
  },
});
