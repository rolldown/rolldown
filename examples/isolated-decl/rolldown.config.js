import { defineConfig } from 'rolldown';
import IsolatedDecl from 'unplugin-isolated-decl/rolldown';

export default defineConfig({
  input: 'src/main.ts',
  plugins: [IsolatedDecl()],
});
