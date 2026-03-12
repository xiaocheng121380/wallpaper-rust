import { createRouter, createWebHashHistory } from "vue-router";

import LibraryView from "../views/LibraryView.vue";
import DiscoverView from "../views/DiscoverView.vue";
import SettingsView from "../views/SettingsView.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", redirect: "/library" },
    { path: "/library", name: "library", component: LibraryView },
    { path: "/discover", name: "discover", component: DiscoverView },
    { path: "/settings", name: "settings", component: SettingsView }
  ],
});
