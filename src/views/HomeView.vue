<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  NButton,
  NCard,
  NEmpty,
  NImage,
  NScrollbar,
  NSpace,
  NText,
  NH2,
  NH3,
} from 'naive-ui'

const { t } = useI18n()
const router = useRouter()

interface Wallpaper {
  id: string
  title: string
  thumbnail: string
}

const currentWallpaper = ref<Wallpaper | null>(null)
const recentImports = ref<Wallpaper[]>([])

function goToLibrary() {
  router.push('/library')
}
</script>

<template>
  <NScrollbar class="home-container">
    <!-- 顶部 Banner -->
    <NCard class="banner-section" :bordered="false">
      <NSpace :size="24" align="center" :wrap="false">
        <div class="banner-preview">
          <NImage v-if="currentWallpaper" :src="currentWallpaper.thumbnail" :alt="currentWallpaper.title"
            object-fit="cover" preview-disabled width="320" />
          <NEmpty v-else description="" style="padding: 40px 0">
            <template #icon>
              <NText style="font-size: 48px; opacity: 0.3">🖼️</NText>
            </template>
          </NEmpty>
        </div>
        <NSpace vertical :size="12">
          <NH2 style="margin: 0">{{ t('currentWallpaper') }}</NH2>
          <NText v-if="currentWallpaper" depth="2">{{ currentWallpaper.title }}</NText>
          <NText v-else depth="3">{{ t('noWallpapersHint') }}</NText>
          <NButton type="primary" @click="goToLibrary">
            {{ t('openLibrary') }}
          </NButton>
        </NSpace>
      </NSpace>
    </NCard>

    <!-- 最近导入 -->
    <NSpace vertical class="scroll-section">
      <NH3 style="margin: 0">{{ t('recentImport') }}</NH3>
      <div class="scroll-container" v-if="recentImports.length > 0">
        <NCard v-for="item in recentImports" :key="item.id" class="thumb-card" hoverable :bordered="false">
          <template #cover>
            <NImage :src="item.thumbnail" :alt="item.title" object-fit="cover" preview-disabled width="180"
              height="101" />
          </template>
          <NText class="thumb-title">{{ item.title }}</NText>
        </NCard>
      </div>
      <NEmpty v-else :description="t('noWallpapers')" />
    </NSpace>
  </NScrollbar>
</template>

<style scoped>
.home-container {
  height: 100%;
  padding: 20px;
}

.banner-section {
  margin-bottom: 24px;
  background: rgba(255, 255, 255, 0.03);
}

.banner-preview {
  width: 320px;
  min-width: 320px;
  aspect-ratio: 16/9;
  border-radius: 8px;
  overflow: hidden;
  background: rgba(255, 255, 255, 0.05);
  display: flex;
  align-items: center;
  justify-content: center;
}

.scroll-section {
  padding: 0 4px;
}

.scroll-container {
  display: flex;
  gap: 12px;
  overflow-x: auto;
  padding-bottom: 8px;
}

.scroll-container::-webkit-scrollbar {
  height: 6px;
}

.scroll-container::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.1);
  border-radius: 3px;
}

.thumb-card {
  width: 180px;
  min-width: 180px;
  background: rgba(255, 255, 255, 0.03);
}

.thumb-title {
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  display: block;
}
</style>
