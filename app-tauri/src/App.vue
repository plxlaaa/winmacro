<script setup lang="ts">
import { ref, onMounted } from "vue";
import { api, onEvent } from "./api";
import Library from "./views/Library.vue";
import SettingsView from "./views/Settings.vue";

const page = ref<"library" | "settings">("library");
const recording = ref(false);
const playing = ref(false);

function refreshFlags() {
  api.isRecording().then((v) => (recording.value = v));
  api.isPlaying().then((v) => (playing.value = v));
}

onMounted(async () => {
  refreshFlags();
  await onEvent("rec:started", () => (recording.value = true));
  await onEvent("rec:done", () => {
    recording.value = false;
    refreshFlags();
  });
  await onEvent("play:started", () => (playing.value = true));
  await onEvent("play:done", () => (playing.value = false));
  await onEvent("play:stopped", () => (playing.value = false));
});
</script>

<template>
  <div class="layout">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-logo">▶</div>
        <div>
          <div class="brand-name">WinMacro</div>
          <div class="brand-sub">脚本录制 · 回放</div>
        </div>
      </div>
      <div
        class="nav-item"
        :class="{ active: page === 'library' }"
        @click="page = 'library'"
      >
        <span>🎬</span><span>录制库</span>
      </div>
      <div
        class="nav-item"
        :class="{ active: page === 'settings' }"
        @click="page = 'settings'"
      >
        <span>⚙️</span><span>设置</span>
      </div>
      <div class="sidebar-foot">
        <span class="hint-kbd">F10</span> 开始 / 结束录制<br />
        <span class="hint-kbd">F11</span> 停止回放
      </div>
    </aside>
    <main class="main">
      <Library v-if="page === 'library'" :recording="recording" :playing="playing" />
      <SettingsView v-else @saved="refreshFlags" />
    </main>
  </div>
</template>
