<script setup lang="ts">
import { computed, ref } from 'vue';

declare const __ROLLDOWN_VERSION__: string;

const supportedVersionMessage = {
  color: 'var(--vp-c-brand-1)',
  text: 'supported',
};
const notSupportedVersionMessage = {
  color: 'var(--vp-c-danger-1)',
  text: 'not supported',
};

// Latest minor of each previous major. Empty until Rolldown ships a 2.x.
const previousMajorLatestMinors: Record<string, string> = {};

const parsedRolldownVersion = parseVersion(__ROLLDOWN_VERSION__)!;
const supportInfo = computeSupportInfo(parsedRolldownVersion);

const checkedVersion = ref(`${parsedRolldownVersion.major}.0.0`);
const checkedResult = computed(() => {
  const version = checkedVersion.value;
  if (!isValidRolldownVersion(version)) return notSupportedVersionMessage;

  const parsedVersion = parseVersion(checkedVersion.value);
  if (!parsedVersion) return notSupportedVersionMessage;

  const satisfies = (targetVersion: string) => {
    const compared = parseVersion(targetVersion)!;
    return parsedVersion.major === compared.major && parsedVersion.minor >= compared.minor;
  };
  const satisfiesOneSupportedVersion =
    parsedVersion.major >= parsedRolldownVersion.major ||
    supportInfo.regularPatches.some(satisfies) ||
    supportInfo.importantFixes.some(satisfies) ||
    supportInfo.securityPatches.some(satisfies);

  return satisfiesOneSupportedVersion ? supportedVersionMessage : notSupportedVersionMessage;
});

function parseVersion(version: string) {
  let [major, minor, patch] = version.split('.').map((v) => {
    const num = /^\d+$/.exec(v)?.[0];
    return num ? parseInt(num) : null;
  });
  if (major == null) return null;
  minor ??= 0;
  patch ??= 0;
  return { major, minor, patch };
}

function computeSupportInfo(version: NonNullable<ReturnType<typeof parseVersion>>) {
  const { major, minor } = version;
  const f = (versions: string[]) => {
    return versions
      .map((v) => previousMajorLatestMinors[v] ?? v)
      .filter((version) => {
        if (!isValidRolldownVersion(version)) return false;
        if (/-\d/.test(version)) return false;
        return true;
      });
  };

  return {
    regularPatches: f([`${major}.${minor}`]),
    importantFixes: f([`${major - 1}`, `${major}.${minor - 1}`]),
    securityPatches: f([`${major - 2}`, `${major}.${minor - 2}`]),
  };
}

function versionsToText(versions: string[]) {
  versions = versions.map((v) => `<code>rolldown@${v}</code>`);
  if (versions.length === 0) return '';
  if (versions.length === 1) return versions[0];
  return versions.slice(0, -1).join(', ') + ' and ' + versions[versions.length - 1];
}

function isValidRolldownVersion(version: string) {
  if (version.length === 1) version += '.';
  // Rolldown 0.x was pre-stable and is no longer maintained.
  if (version.startsWith('0.')) return false;
  return true;
}
</script>

<template>
  <div>
    <ul>
      <li v-if="supportInfo.regularPatches.length">
        Regular patches are released for
        <span v-html="versionsToText(supportInfo.regularPatches)"></span>.
      </li>
      <li v-if="supportInfo.importantFixes.length">
        Important fixes and security patches are backported to
        <span v-html="versionsToText(supportInfo.importantFixes)"></span>.
      </li>
      <li v-if="supportInfo.securityPatches.length">
        Security patches are also backported to
        <span v-html="versionsToText(supportInfo.securityPatches)"></span>.
      </li>
      <li>
        All versions before these are no longer supported. Users should upgrade to receive updates.
      </li>
    </ul>
    <p>
      If you're using Rolldown
      <input class="checked-input" type="text" v-model="checkedVersion" placeholder="0.0.0" />, it
      is <strong :style="{ color: checkedResult.color }">{{ checkedResult.text }}</strong
      >.
    </p>
  </div>
</template>

<style scoped>
.checked-input {
  display: inline-block;
  padding: 0px 5px;
  width: 100px;
  color: var(--vp-c-text-1);
  background: var(--vp-c-bg-soft);
  font-size: var(--vp-code-font-size);
  font-family: var(--vp-font-family-mono);
  border: 1px solid var(--vp-c-divider);
  border-radius: 5px;
  transition: border-color 0.1s;
}

.checked-input:focus,
.checked-input:hover {
  border-color: var(--vp-c-brand);
}
</style>
