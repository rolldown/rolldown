export default async function loadConfig() {
  const { input } = await import('./dynamic-function-value');
  return { input };
}
