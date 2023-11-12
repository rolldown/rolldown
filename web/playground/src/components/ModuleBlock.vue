<script setup lang="ts">
import CodeBlock from './CodeBlock.vue';
import { defineProps } from 'vue'

defineProps({
  title: String,
  code: String,
  autoFocus: {
    type: Boolean,
    required: false
  }
})

</script>

<template>
  <div>
    <div class="title" contenteditable @input="$emit('title', $event)">{{ title }}</div>
    <CodeBlock :code="code" @code="$emit('code', $event)"/>
  </div>
</template>

<style>
.title {
  margin: auto;
}
</style>
