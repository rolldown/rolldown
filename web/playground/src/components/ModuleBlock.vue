<script setup lang="ts">
import CodeBlock from './CodeBlock.vue'
import { defineProps, onMounted, ref } from 'vue'

const props = defineProps({
  title: {
    type: String,
    required: true,
  },
  code: {
    type: String,
    required: true,
  },
  autoFocus: {
    type: Boolean,
    required: false,
  },
  isEntry: {
    type: Boolean,
    required: false,
  },
  canModifyEntry: {
    type: Boolean,
    required: false,
  },
  readonly: {
    type: Boolean,
    required: false,
  },
})

const input = ref(null)

onMounted(() => {
  if (props.autoFocus && input.value) {
    const [basename, _] = props.title.split('.')
    const target = input.value as any
    var range = document.createRange()

    // Select the text nodes within the div (startNode, startOffset, endNode, endOffset)
    range.setStart(target.childNodes[0], 0)
    range.setEnd(target.childNodes[0], basename.length)
    getSelection()?.removeAllRanges()

    // Add the new range to the selection
    getSelection()?.addRange(range)
    target.focus()
  }
})
</script>

<template>
  <div>
    <div class="title-container">
      <div
        class="title"
        :class="{ 'is-entry': !!isEntry }"
        ref="input"
        contenteditable
        @input="$emit('title', $event)"
      >
        {{ title }}
      </div>
      <button
        class="entry-flag"
        v-show="canModifyEntry"
        @click="$emit('isEntry')"
      >
        entry
      </button>
    </div>

    <CodeBlock :code="code" @code="$emit('code', $event)" :readonly="readonly" />
  </div>
</template>

<style>
.title-container {
  display: flex;
  justify-content: space-between;
}

.title.is-entry {
  background: #5672cdaa;
  color: white;
}

.title {
  flex: 1;
}
.entry-flag {
  outline: none;
  border: none;
}
</style>
