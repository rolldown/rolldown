import shared from './shared.cjs';
export const value = shared.value;
export async function load() {
  return shared === (await import('./dynamic.cjs')).default;
}
