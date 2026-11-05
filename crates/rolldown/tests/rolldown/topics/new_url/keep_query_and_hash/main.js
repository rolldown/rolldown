// The query and fragment are part of the URL and must survive the rewrite.
export const query = new URL('./sprite.svg?v=1', import.meta.url).href;
export const hash = new URL('./sprite.svg#icon', import.meta.url).href;
export const both = new URL('./sprite.svg?v=1#icon', import.meta.url).href;
export const plain = new URL('./sprite.svg', import.meta.url).href;
