<script setup lang="ts">
import { useData } from 'vitepress';
import RolldownTheme from '@voidzero-dev/vitepress-theme/src/rolldown';
import { computed } from 'vue';

const { Layout } = RolldownTheme;
const { frontmatter } = useData();

const isDoc = computed(
  () => frontmatter.value.layout === undefined || frontmatter.value.layout === 'doc',
);
</script>

<template>
  <Layout>
    <template v-if="isDoc" #doc-after>
      <FeedbackWidget />
    </template>
    <template v-if="isDoc" #aside-outline-after>
      <FeedbackWidgetCompact />
    </template>
  </Layout>
</template>
