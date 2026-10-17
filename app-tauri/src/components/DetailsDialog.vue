<script setup lang="ts">
import { ref, computed } from "vue";
import {
  KIND_LABELS,
  BUTTON_LABELS,
  fmtDuration,
  type Recording,
  type InputEvent,
} from "../api";

const props = defineProps<{ recording: Recording }>();
const emit = defineEmits<{ (e: "close"): void }>();

const PAGE = 500;
const page = ref(0);

const shown = computed(() =>
  props.recording.events.slice(0, page.value * PAGE + PAGE),
);
const hasMore = computed(() => shown.value.length < props.recording.events.length);

function timeText(ev: InputEvent): string {
  return (ev.t_ms / 1000).toFixed(2) + "s";
}
function posText(ev: InputEvent): string {
  if (ev.x == null || ev.y == null) return "—";
  return `${ev.x}, ${ev.y}`;
}
function extraText(ev: InputEvent): string {
  if (ev.kind === "mouse_scroll") {
    const dy = ev.dy ?? 0;
    return `滚动 ${dy > 0 ? "↑" : "↓"} ${Math.abs(dy)}`;
  }
  if (ev.kind === "mouse_down" || ev.kind === "mouse_up")
    return BUTTON_LABELS[ev.button ?? "left"] ?? "左键";
  if (ev.kind === "key_down" || ev.kind === "key_up") return ev.key ?? "—";
  return "";
}
</script>

<template>
  <div class="overlay-mask" @click.self="emit('close')">
    <div class="dialog">
      <div class="dialog-head">
        <div class="dialog-title">{{ recording.name }}</div>
        <button class="btn ghost" @click="emit('close')">✕</button>
      </div>
      <div style="padding: 0 22px 12px; color: var(--muted); font-size: 12.5px">
        {{ recording.created_at.replace("T", " ") }} · {{ recording.events.length.toLocaleString() }} 步 ·
        {{ fmtDuration(recording.duration_ms) }} ·
        分辨率 {{ recording.screen_width ? `${recording.screen_width}×${recording.screen_height}` : "未记录" }}
      </div>
      <div class="dialog-body">
        <table class="steps">
          <thead>
            <tr>
              <th style="width: 60px">#</th>
              <th style="width: 110px">类型</th>
              <th style="width: 90px">时间</th>
              <th style="width: 130px">坐标</th>
              <th>详情</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(ev, i) in shown" :key="i">
              <td class="idx">{{ String(i + 1).padStart(4, "0") }}</td>
              <td>{{ KIND_LABELS[ev.kind] ?? ev.kind }}</td>
              <td>{{ timeText(ev) }}</td>
              <td>{{ posText(ev) }}</td>
              <td>{{ extraText(ev) }}</td>
            </tr>
          </tbody>
        </table>
        <div v-if="hasMore" style="text-align: center; padding: 14px">
          <button class="btn" @click="page++">
            加载更多（已显示 {{ shown.length }} /
            {{ recording.events.length.toLocaleString() }}）
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
