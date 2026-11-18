export const bold = 'font-weight: bold';

export function throw_error(code, message) {
  const error = new Error(`${code}\n${message}\nhttps://example.com/${code}`);
  error.name = 'Rich error';
  throw error;
}
