import { BindingMagicString, defineConfig } from 'rolldown';

export default defineConfig({
  input: './index.js',
  plugins: [
    {
      name: 'test',
      transform(code) {
        let a = new BindingMagicString(code);
        a.append('\nconsole.log("appended")\n');
        console.log(`this.sendMagicString: `, this.sendMagicString(a));
      },
    },
  ],
});
