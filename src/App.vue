<script setup lang="ts">
import {
  NConfigProvider,
  NSpace,
  NText,
  NLayout,
  NLayoutHeader,
  NMenu,
  NButton,
  darkTheme,
  type GlobalThemeOverrides,
  type MenuOption,
} from 'naive-ui'
import { computed, ref, watchEffect, provide, h } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { NIcon } from 'naive-ui'
import {
  CloseOutline,
  ContractOutline,
  ExpandOutline,
  ImagesOutline,
  RemoveOutline,
  SearchOutline,
  SettingsOutline,
} from '@vicons/ionicons5'
import { useSettingsStore } from './stores/settings'
import InspectorPanel from './components/InspectorPanel.vue'
import FirstCloseDialog from './components/FirstCloseDialog.vue'
import AppAlert from './components/AppAlert.vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import logoUrl from './assets/logo.png'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const settingsStore = useSettingsStore()

settingsStore.initialize()

const viewMode = ref<'grid' | 'list'>('grid')
const selectedWallpaper = ref<any>(null)

const alertVisible = ref(false)
const alertMessage = ref('')


provide('viewMode', viewMode)
provide('selectedWallpaper', selectedWallpaper)

provide('showAlert', (message: string) => {
  alertMessage.value = message
  alertVisible.value = true
})

const activeKey = computed(() => {
  const p = route.path
  if (p.startsWith('/library')) return 'library'
  if (p.startsWith('/discover')) return 'discover'
  if (p.startsWith('/settings')) return 'settings'
  return 'library'
})

const navRoutes: Record<string, string> = {
  library: '/library',
  discover: '/discover',
  settings: '/settings',
}

const menuOptions = computed<MenuOption[]>(() => [
  {
    label: t('library'),
    key: 'library',
    icon: () => h(NIcon, null, { default: () => h(ImagesOutline) }),
  },
  {
    label: t('discover'),
    key: 'discover',
    icon: () => h(NIcon, null, { default: () => h(SearchOutline) }),
  },
  {
    label: t('settings'),
    key: 'settings',
    icon: () => h(NIcon, null, { default: () => h(SettingsOutline) }),
  },
])

function navigate(path: string) {
  router.push(path)
}

function handleMenuSelect(key: string) {
  const path = navRoutes[key]
  if (path) router.push(path)
}

const isDark = computed(() => {
  if (settingsStore.themeMode === 'dark') return true
  if (settingsStore.themeMode === 'light') return false
  if (typeof window !== 'undefined') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches
  }
  return false
})

const theme = computed(() => (isDark.value ? darkTheme : null))

const themeOverrides = computed<GlobalThemeOverrides>(() => {
  const primary = settingsStore.primaryColor || '#18a058'
  return {
    common: {
      primaryColor: primary,
      primaryColorHover: primary,
      primaryColorPressed: primary,
      primaryColorSuppl: primary,
    },
  }
})

const showInspector = computed(() => {
  return activeKey.value === 'library'
})

const isMaximized = ref(false)

function getWindow() {
  try {
    return getCurrentWindow()
  } catch {
    return null
  }
}

async function refreshWindowState() {
  const w = getWindow()
  if (!w) return
  try {
    isMaximized.value = await w.isMaximized()
  } catch {
    // ignore
  }
}

async function handleMinimize() {
  const w = getWindow()
  if (!w) return
  await w.minimize()
}

async function handleToggleMaximize() {
  const w = getWindow()
  if (!w) return
  await w.toggleMaximize()
  await refreshWindowState()
}

async function handleClose() {
  const w = getWindow()
  if (!w) return
  await w.close()
}

function handleTitlebarMouseDown(e: MouseEvent) {
  const w = getWindow()
  if (!w) return
  if (e.buttons !== 1) return

  e.preventDefault()

  if (e.detail === 2) {
    w.toggleMaximize()
      .then(() => refreshWindowState())
      .catch(() => {
        // ignore
      })
    return
  }

  w.startDragging().catch(() => {
    // ignore
  })
}

refreshWindowState()

watchEffect(() => {
  document.body.style.backgroundColor = isDark.value ? '#0d0d0f' : '#f5f5f5'
  document.body.style.color = isDark.value ? '#ffffffd9' : '#333'
})
</script>

<template>
  <NConfigProvider :theme="theme" :theme-overrides="themeOverrides">
    <NLayout class="app-container" :class="{ dark: isDark }" :native-scrollbar="false">
      <!-- 顶部栏 TopBar -->
      <NLayoutHeader class="top-bar" bordered>
        <div class="titlebar">
          <div class="titlebar-drag">
            <NSpace
              align="center"
              :wrap="false"
              class="brand"
              @mousedown="handleTitlebarMouseDown"
              @click="navigate('/library')"
            >
              <img class="app-logo" :src="logoUrl" alt="logo" />
              <NText strong class="app-name">{{ t('welcome') }}</NText>
            </NSpace>
          </div>

          <div class="titlebar-center">
            <NMenu mode="horizontal" :value="activeKey" :options="menuOptions" @update:value="handleMenuSelect" />
          </div>

          <div class="titlebar-controls">
            <NButton quaternary size="small" class="win-btn" @click="handleMinimize">
              <template #icon>
                <NIcon><RemoveOutline /></NIcon>
              </template>
            </NButton>
            <NButton quaternary size="small" class="win-btn" @click="handleToggleMaximize">
              <template #icon>
                <NIcon>
                  <component :is="isMaximized ? ContractOutline : ExpandOutline" />
                </NIcon>
              </template>
            </NButton>
            <NButton quaternary size="small" class="win-btn win-btn-close" @click="handleClose">
              <template #icon>
                <NIcon><CloseOutline /></NIcon>
              </template>
            </NButton>
          </div>
        </div>
      </NLayoutHeader>

      <!-- 主体区域 -->
      <div class="main-body" :class="{ 'has-inspector': showInspector }">
        <!-- 左侧：资源区（页面内部自己滚动，例如 LibraryView 的 NScrollbar） -->
        <div class="left-pane">
          <div class="router-view-container">
            <RouterView v-slot="{ Component, route }">
              <Transition name="page" mode="out-in">
                <KeepAlive :include="['LibraryView', 'DiscoverView', 'SettingsView']">
                  <component :is="Component" :key="route.name as any" />
                </KeepAlive>
              </Transition>
            </RouterView>
          </div>
        </div>

        <!-- 右侧：详情栏（固定窗口高度，不滚动） -->
        <InspectorPanel v-if="showInspector" v-model:selectedWallpaper="selectedWallpaper" />
      </div>
    </NLayout>
    <AppAlert v-model:show="alertVisible" :message="alertMessage" />
    <FirstCloseDialog />
  </NConfigProvider>
</template>

<style scoped>
.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background: #f5f5f5;
  color: #333;
}

.app-container :deep(.n-layout-scroll-container) {
  overflow: hidden !important;
}

.app-container.dark {
  background: #0d0d0f;
  color: #ffffffd9;
}

/* 顶部栏 */
.top-bar {
  height: 48px;
  min-height: 48px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px;
  background: rgba(255, 255, 255, 0.03);
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.titlebar {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  gap: 12px;
  user-select: none;
}

.titlebar-drag {
  display: flex;
  align-items: center;
  min-width: 160px;
  height: 100%;
}

.titlebar-center {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  height: 100%;
}

.titlebar-controls {
  display: flex;
  align-items: center;
  gap: 4px;
}

.win-btn {
  width: 34px;
  height: 30px;
}

.win-btn :deep(.n-button__icon) {
  margin: 0;
}

.win-btn-close:hover {
  background: rgba(255, 77, 79, 0.14);
}

.top-bar-left {
  display: flex;
  align-items: center;
  gap: 24px;
  flex-shrink: 0;
}

.brand {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}

.app-logo {
  width: 18px;
  height: 18px;
  display: block;
}

.logo {
  font-size: 22px;
  line-height: 1;
}

.app-name {
  font-size: 15px;
  font-weight: 600;
}

.nav-links {
  display: flex;
  align-items: center;
  gap: 4px;
}

.nav-link {
  padding: 6px 12px;
  font-size: 13px;
  font-weight: 500;
  color: inherit;
  opacity: 0.6;
  cursor: pointer;
  border-radius: 6px;
  transition: all 0.2s ease;
  text-decoration: none;
}

.nav-link:hover {
  opacity: 1;
  background: rgba(255, 255, 255, 0.06);
}

.nav-link.active {
  opacity: 1;
  background: rgba(255, 255, 255, 0.1);
}

.top-bar-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

/* 主体区域 */
.main-body {
  flex: 1;
  display: flex;
  overflow: hidden;
  position: relative;
  height: calc(100vh - 48px);
}

.main-body.has-inspector {
  padding-right: 280px;
}

.left-pane {
  flex: 1;
  min-width: 0;
  min-height: 0;
  height: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* 中央内容区 */
.main-content {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.router-view-container {
  flex: 1;
  min-height: 0;
  display: flex;
  height: 100%;
}

.router-view-container> :deep(*) {
  flex: 1;
  min-height: 0;
  height: 100%;
}

/* 右侧详情栏 */
.inspector {
  width: 280px;
  min-width: 280px;
  display: flex;
  flex-direction: column;
  background: rgba(255, 255, 255, 0.02);
  border-left: 1px solid rgba(255, 255, 255, 0.06);
  transition: transform 0.3s ease;
}

.inspector-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.inspector-title {
  font-size: 14px;
  font-weight: 600;
}

.inspector-close {
  background: none;
  border: none;
  color: inherit;
  cursor: pointer;
  opacity: 0.5;
  font-size: 14px;
  padding: 4px;
}

.inspector-close:hover {
  opacity: 1;
}

.inspector-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 24px;
}

.empty-icon {
  font-size: 48px;
  opacity: 0.3;
  margin-bottom: 12px;
}

.empty-text {
  font-size: 13px;
  opacity: 0.5;
  text-align: center;
}

.inspector-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 16px;
}

.inspector-scroll {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.preview-large {
  width: 100%;
  aspect-ratio: 16/9;
  border-radius: 8px;
  overflow: hidden;
  background: rgba(255, 255, 255, 0.05);
  margin-bottom: 16px;
}

.preview-large :deep(.n-image),
.preview-large :deep(.n-image img) {
  width: 100%;
  height: 100%;
}

.preview-large :deep(.n-image img) {
  object-fit: cover;
}

.preview-large img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.info-section {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 16px;
}

.info-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.info-row label {
  font-size: 12px;
  opacity: 0.5;
}

.info-row.readonly span {
  font-size: 13px;
}

.file-path {
  word-break: break-all;
  font-size: 12px !important;
  opacity: 0.7;
}

.inspector-actions {
  margin-top: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

/* 页面切换过渡动画 */
.page-enter-active,
.page-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}

.page-enter-from {
  opacity: 0;
  transform: translateX(10px);
}

.page-leave-to {
  opacity: 0;
  transform: translateX(-10px);
}
</style>
