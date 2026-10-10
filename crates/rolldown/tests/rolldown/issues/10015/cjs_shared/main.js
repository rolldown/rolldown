import shared from './shared.cjs';

export const POST = async () => {
  const { default: dynamic } = await import('./dynamic.cjs');
  return { same: shared === dynamic.get(), value: shared.tag + ':' + dynamic.run() };
};
