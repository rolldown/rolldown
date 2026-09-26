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

const parsedRolldownVersion = parseVersion(__ROLLDOWN_VERSION__)!;
const currentMinor = `${parsedRolldownVersion.major}.${parsedRolldownVersion.minor}`;
const previousMinor =
  parsedRolldownVersion.minor > 0
    ? `${parsedRolldownVersion.major}.${parsedRolldownVersion.minor - 1}`
    : undefined;

const checkedVersion = ref(`${parsedRolldownVersion.major}.0.0`);
const checkedResult = computed(() => {
  const version = checkedVersion.value;
  if (!isValidRolldownVersion(version)) return notSupportedVersionMessage;

  const parsedVersion = parseVersion(version);
  if (!parsedVersion) return notSupportedVersionMessage;

  const supported =
    parsedVersion.major > parsedRolldownVersion.major ||
    (parsedVersion.major === parsedRolldownVersion.major &&
      parsedVersion.minor >= parsedRolldownVersion.minor);

  return supported ? supportedVersionMessage : notSupportedVersionMessage;
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
      <li>
        Regular fixes are released for <code>rolldown@{{ currentMinor }}</code
        >.
      </li>
      <li v-if="previousMinor">
        If a serious security issue is found, a fix may be backported to
        <code>rolldown@{{ previousMinor }}</code
        >.
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
