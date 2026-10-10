import shared from './shared.cjs';

export default async () => (await import('./dynamic.cjs')).default === shared;
