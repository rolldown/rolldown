// A self-signed certificate for the replay server. Chrome runs with
// --ignore-certificate-errors (a throwaway profile, the replay hosts only), so the
// names in it do not matter; TLS is needed because Chrome speaks HTTP/2 only
// over TLS, and the recorded site used HTTP/2.

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export function replayCertificate(dir = path.join(os.tmpdir(), 'perf-kit-replay', 'cert')) {
  const keyFile = path.join(dir, 'key.pem');
  const certFile = path.join(dir, 'cert.pem');
  if (!fs.existsSync(keyFile) || !fs.existsSync(certFile)) {
    fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
    try {
      execFileSync(
        'openssl',
        [
          'req',
          '-x509',
          '-newkey',
          'ec',
          '-pkeyopt',
          'ec_paramgen_curve:prime256v1',
          '-nodes',
          '-days',
          '365',
          '-subj',
          '/CN=perf-kit-replay',
          '-addext',
          'subjectAltName=DNS:localhost,IP:127.0.0.1',
          '-keyout',
          keyFile,
          '-out',
          certFile,
        ],
        { stdio: ['ignore', 'ignore', 'pipe'] },
      );
    } catch (err) {
      throw new Error(`replay needs openssl to make a local TLS certificate: ${err.message}`);
    }
  }
  return { key: fs.readFileSync(keyFile), cert: fs.readFileSync(certFile) };
}
