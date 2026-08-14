export default async function run() {
  const { result } = await import('./barrel.js');
  return result;
}
