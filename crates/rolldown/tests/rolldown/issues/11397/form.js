import messages from './translations.js';

export const translations = messages;
export const transferForm = () => ({ name: 'transfer' });
export let count = 0;
export function increment() {
  count++;
}
