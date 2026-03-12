import { computed } from 'vue'
import { useRoute } from 'vue-router'

export function useAppNav() {
  const route = useRoute()

  const activeKey = computed(() => {
    const p = route.path
    if (p.startsWith('/library')) return 'library'
    if (p.startsWith('/discover')) return 'discover'
    if (p.startsWith('/settings')) return 'settings'
    return 'home'
  })

  return { activeKey }
}
