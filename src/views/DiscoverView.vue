<script setup lang="ts">
defineOptions({ name: 'DiscoverView' })

import { ref, onMounted, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { db } from '../utils/db'
import { api } from '../api'
import { logger } from '../utils/logger'
import {
  NButton,
  NInput,
  NText,
  NCard,
  NScrollbar,
  NIcon,
} from 'naive-ui'
import { LinkOutline } from '@vicons/ionicons5'

const { t } = useI18n()

const urlInput = ref('')
const isLoading = ref(false)
const historyUrls = ref<string[]>([])
const showHistory = ref(false)
const previewUrl = ref('')
const urlInputRef = ref<InstanceType<typeof NInput> | null>(null)
const suppressOpenHistory = ref(false)
const isHoverSearch = ref(false)
const isHoverHistory = ref(false)
const closeHistoryTimer = ref<number | null>(null)

onMounted(async () => {
  const h = await db.get<{ schema_version: number; urls: string[] }>('discover_history', {
    schema_version: 1,
    urls: [],
  })
  historyUrls.value = h.urls || []
})

async function persistHistory(nextUrls: string[]) {
  historyUrls.value = nextUrls
  await db.set('discover_history', { schema_version: 1, urls: nextUrls })
}

function handlePickHistory(url: string) {
  urlInput.value = url
  suppressOpenHistory.value = true
  showHistory.value = false
  nextTick(() => {
    ; (urlInputRef.value as any)?.blur?.()
  })
  window.setTimeout(() => {
    suppressOpenHistory.value = false
  }, 200)
  handleLoadUrl()
}

function handleDeleteHistory(url: string) {
  const nextUrls = historyUrls.value.filter(u => u !== url)
  persistHistory(nextUrls)
}

function handleFocus() {
  if (suppressOpenHistory.value) return
  showHistory.value = true
}

function handleBlur() {
  window.setTimeout(() => {
    showHistory.value = false
  }, 150)
}

function clearCloseHistoryTimer() {
  if (closeHistoryTimer.value !== null) {
    window.clearTimeout(closeHistoryTimer.value)
    closeHistoryTimer.value = null
  }
}

function scheduleCloseHistory() {
  clearCloseHistoryTimer()
  closeHistoryTimer.value = window.setTimeout(() => {
    if (isHoverSearch.value || isHoverHistory.value) return
    showHistory.value = false
    nextTick(() => {
      ; (urlInputRef.value as any)?.blur?.()
    })
  }, 120)
}

function handleSearchMouseEnter() {
  isHoverSearch.value = true
  clearCloseHistoryTimer()
}

function handleSearchMouseLeave() {
  isHoverSearch.value = false
  scheduleCloseHistory()
}

function handleHistoryMouseEnter() {
  isHoverHistory.value = true
  clearCloseHistoryTimer()
}

function handleHistoryMouseLeave() {
  isHoverHistory.value = false
  scheduleCloseHistory()
}

function handleLoadUrl() {
  if (!urlInput.value.trim()) return
  isLoading.value = true
  const url = urlInput.value.trim()
  previewUrl.value = url
  const deduped = [url, ...historyUrls.value.filter(u => u !== url)].slice(0, 30)
  persistHistory(deduped)
  isLoading.value = false
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter') {
    handleLoadUrl()
  }
}

async function handleOpenInBrowser() {
  if (!urlInput.value.trim()) return
  try {
    const result = await api.systemOpenPath(urlInput.value.trim())
    if (!result.ok) {
      logger.error('在浏览器中打开链接失败:', result.error)
    }
  } catch (e) {
    logger.error('在浏览器中打开链接失败:', e)
  }
}
</script>

<template>
  <div class="discover-container">
    <div class="discover-header">
      <NCard class="search-card" :bordered="false" size="small">
        <div class="search-row">
          <div class="search-toolbar">
            <div class="search-input-wrap" @mouseenter="handleSearchMouseEnter" @mouseleave="handleSearchMouseLeave">
              <NInput ref="urlInputRef" v-model:value="urlInput" clearable size="large" placeholder="输入链接进行搜索..."
                @keydown="handleKeydown" @focus="handleFocus" @blur="handleBlur">
                <template #prefix>
                  <NIcon :component="LinkOutline" />
                </template>
              </NInput>

              <transition name="history-fade">
                <div v-show="showHistory && historyUrls.length" class="history-dropdown" @mousedown.prevent
                  @mouseenter="handleHistoryMouseEnter" @mouseleave="handleHistoryMouseLeave">
                  <NCard size="small" :bordered="true" class="history-card">
                    <NScrollbar style="max-height: 260px">
                      <div v-for="u in historyUrls" :key="u" class="history-item" @click="handlePickHistory(u)">
                        <div class="history-text">{{ u }}</div>
                        <NButton size="tiny" quaternary type="error" @click.stop="handleDeleteHistory(u)">
                          删除
                        </NButton>
                      </div>
                    </NScrollbar>
                  </NCard>
                </div>
              </transition>
            </div>

            <NButton type="primary" size="large" @click="handleLoadUrl" :loading="isLoading">
              {{ t('search') }}
            </NButton>
            <NButton size="large" @click="handleOpenInBrowser" :disabled="!urlInput.trim()">
              {{ t('openInBrowser') }}
            </NButton>
          </div>
        </div>
      </NCard>
    </div>

    <div v-if="previewUrl" class="preview-area">
      <iframe :src="previewUrl" class="preview-frame"
        sandbox="allow-scripts allow-same-origin allow-downloads allow-popups" />
    </div>

    <div v-else class="preview-empty">
      <NText depth="3">输入链接点击搜索，下方将展示网页预览</NText>
    </div>
  </div>
</template>

<style scoped>
.discover-container {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.discover-header {
  padding: 16px 20px;
}

.search-card {
  border: 1px solid rgba(255, 255, 255, 0.08);
}

.preview-empty {
  flex: 1;
  padding: 0 20px 20px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.preview-area {
  flex: 1;
  padding: 0 20px 20px;
  overflow: hidden;
}

.preview-frame {
  width: 100%;
  height: 100%;
  border: none;
  border-radius: 10px;
  background: #fff;
}

.search-row {
  position: relative;
}

.search-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
}

.search-input-wrap {
  position: relative;
  flex: 1;
  min-width: 0;
}

.history-dropdown {
  position: absolute;
  left: 0;
  right: 0;
  top: calc(100% + 8px);
  z-index: 50;
}

.history-dropdown::before {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  top: -8px;
  height: 8px;
}

.history-fade-enter-active,
.history-fade-leave-active {
  transition: opacity 0.16s ease, transform 0.16s ease;
}

.history-fade-enter-from,
.history-fade-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}

.history-card {
  background: rgba(25, 25, 28, 0.98);
}

.history-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 10px;
  border-radius: 6px;
  cursor: pointer;
}

.history-item:hover {
  background: rgba(255, 255, 255, 0.06);
}

.history-text {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.url-input-row {
  display: flex;
  gap: 12px;
  max-width: 600px;
}

.url-input {
  flex: 1;
}

.discover-content {
  flex: 1;
  display: flex;
  overflow: hidden;
  padding: 0 20px 20px;
}

.placeholder-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  padding: 40px;
}

.placeholder-icon {
  font-size: 64px;
  margin-bottom: 20px;
  opacity: 0.3;
}

.placeholder-text {
  font-size: 16px;
  opacity: 0.6;
  margin: 0 0 8px;
}

.placeholder-hint {
  font-size: 13px;
  opacity: 0.4;
  margin: 0;
}
</style>
