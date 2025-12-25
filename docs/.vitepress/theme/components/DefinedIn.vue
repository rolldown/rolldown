<script setup lang="ts">
defineProps<{
  sources: Array<{ link: string; linkName: string }>;
}>();
</script>

<template>
  <div class="defined-in">
    Defined in:
    <span v-for="(source, idx) in sources" :key="source.link"
      ><template v-if="idx !== 0">, </template
      ><a :href="source.link">{{ source.linkName }}</a></span
    >
  </div>
</template>

<style scoped>
.defined-in {
  font-size: 0.875rem;
  color: var(--vp-c-text-secondary);
  margin-top: 0.5rem;
}
</style>
