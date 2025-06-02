export default async () => {
  return await import('./a.js').then(({buildDevConfig}) => buildDevConfig());
};
