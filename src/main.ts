import { createApp } from "vue";
import { createPinia } from "pinia";
import "./style.css";
import App from "./App.vue";
import { router } from "./router";
import { i18n } from "./i18n";
import { useSettingsStore, setupSettingsWatcher } from "./stores/settings";

async function bootstrap() {
  const app = createApp(App);
  const pinia = createPinia();
  app.use(pinia);

  const settingsStore = useSettingsStore(pinia);
  try {
    await settingsStore.initialize();
  } catch {
    // ignore
  }
  const lang = settingsStore.language === "en" ? "en" : "zh-CN";
  i18n.global.locale.value = lang;
  setupSettingsWatcher();

  app.use(router);
  app.use(i18n);
  app.mount("#app");
}

bootstrap();
