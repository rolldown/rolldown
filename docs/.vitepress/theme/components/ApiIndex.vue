<!-- Reference: https://github.com/vuejs/docs/blob/main/src/api/ApiIndex.vue -->
<script setup lang="ts">
// in .vue components or .md pages:
// named import "data" is the resolved static data
// can also import types for type consistency
import { data as apiIndex } from './api.data'
import type { APIReference } from './api.data'
import { ref, computed, onMounted } from 'vue'

const search = ref()
const query = ref('')
const normalize = (s: string) => s.toLowerCase().replace(/-/g, ' ')

onMounted(() => {
    search.value?.focus()
})

const filtered = computed(() => {
    const q = normalize(query.value)
    const matches = (text: string) => normalize(text).includes(q)

    return apiIndex
        .map((section) => {
            // section title match
            if (matches(section.text)) {
                return section
            }

            // filter references
            const matchedReferences = section.items
                .map((item) => {
                    // reference title match
                    if (matches(item.text)) {
                        return item
                    }
                })
                .filter((i) => i)

            return matchedReferences.length
                ? { text: section.text, items: matchedReferences }
                : null
        })
        .filter((i) => i) as APIReference[]
})
</script>

<template>
    <div id="api-index">
        <div class="header">
            <h1>Options & APIs Reference</h1>
            <div class="api-filter">
                <label for="api-filter">Filter</label>
                <input ref="search" type="search" placeholder="Enter keyword" id="api-filter" v-model="query" />
            </div>
        </div>

        <p>This is the automatically generated references for Rolldown's options and APIs. Use the sidebar navigation to browse specific options and APIs. </p>

        <div v-for="section of filtered" :key="section.text" class="api-section">
            <h2 :id="section.anchor">{{ section.text }}</h2>
            <div class="api-references">
                <a v-for="item of section.items" :key="item.text" :href="item.link" class="api-reference">
                    <div>{{ item.text }}</div>
                </a>
            </div>
        </div>

        <div v-if="!filtered.length" class="no-match">
            No API reference matching "{{ query }}" found.
        </div>
    </div>
</template>

<style scoped>
#api-index {
    max-width: 1024px;
    margin: 0px auto;
}

h1,
h2 {
    font-weight: 600;
    line-height: 1;
}

h1,
h2 {
    letter-spacing: -0.02em;
}

h1 {
    font-size: 38px;
}

h2 {
    font-size: 24px;
    color: var(--vt-c-text-1);
    transition: color 0.5s;
    padding-top: 36px;
    border-top: 1px solid;
}

.api-section {
    margin-bottom: 64px;
}

.api-references {
    display: grid;
    gap: 14px;
}

.api-reference {
    break-inside: avoid;
    display: flex;
    flex-direction: column;
    background-color: var(--vt-c-bg-soft);
    border: 1px solid rgba(128, 128, 128, 0.2);
    border-radius: 8px;
    padding: 24px 12px;
    text-decoration: none;
    transition: border-color 0.25s;
    width: 100%;
    box-sizing: border-box;
    font-size: 14px;
    color: var(--vt-c-text-1);
}

.api-reference:hover {
    border-color: var(--vt-c-brand);
}

.header {
    display: flex;
    align-items: center;
    justify-content: space-between;
}

.api-filter {
    display: flex;
    align-items: center;
    justify-content: flex-start;
    gap: 1rem;
}

#api-filter {
    border: 1px solid var(--vt-c-divider);
    border-radius: 8px;
    padding: 6px 12px;
    transition: box-shadow 0.25s ease;
}

#api-filter:focus {
    box-shadow: 0 0 4pt var(--vt-c-brand);
}

.api-filter:focus {
    border-color: var(--vt-c-green-light);
}

.no-match {
    font-size: 1.2em;
    color: var(--vt-c-text-3);
    text-align: center;
    margin-top: 36px;
    padding-top: 36px;
    border-top: 1px solid var(--vt-c-divider-light);
}

@media (max-width: 768px) {
    #api-index {
        padding: 42px 24px;
    }

    h1 {
        font-size: 32px;
        margin-bottom: 24px;
    }

    h2 {
        font-size: 22px;
        margin: 42px 0 32px;
        padding-top: 32px;
    }

    .api-references a {
        font-size: 14px;
    }

    .header {
        display: block;
    }
}

@media (min-width: 768px) {
    .api-references {
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }
}

@media (min-width: 1024px) {
    .api-references {
        grid-template-columns: repeat(3, minmax(0, 1fr));
    }
}
</style>