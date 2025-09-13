export function checkNodeVersion(nodeVersion: string): boolean {
  const currentVersion = nodeVersion.split('.');
  const major = parseInt(currentVersion[0], 10);
  const minor = parseInt(currentVersion[1], 10);
  const isSupported = (major === 20 && minor >= 19) ||
    (major === 22 && minor >= 12) || major > 22;
  return isSupported;
}
