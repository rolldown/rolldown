// `new URL()` decodes its path, so these refer to `my file.txt` and `✓.txt`.
export const spaced = new URL('./my%20file.txt', import.meta.url).href;
export const unicode = new URL('./%E2%9C%93.txt', import.meta.url).href;
