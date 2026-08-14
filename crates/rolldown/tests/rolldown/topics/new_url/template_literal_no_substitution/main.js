// Static template literals should work the same as string literals
export const url = new URL(`./asset1.txt`, import.meta.url);
export const url2 = new URL('./asset2.txt', import.meta.url);
