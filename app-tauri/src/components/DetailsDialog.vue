<script setup lang="ts">
import { ref, computed } from "vue";
import {
  KIND_LABELS,
  BUTTON_LABELS,
  fmtDuration,
  type Recording,
  type InputEvent,
} from "../api";
import Timeline from "./Timeline.vue";

const props = defineProps<{ recording: Recording }>();
const emit = defineEmits<{ (e: "close"): void }>();

const PAGE = 500;
const page = ref(0);
const currentMs = ref<number | null>(null);

const shown = computed(() =>
  props.recording.events.slice(0, page.value * PAGE + PAGE),
);
const hasMore = computed(() => shown.value.length < props.recording.events.length);

/** 跳到指定 t_ms 附近的事件（向下取最接近的事件下标） */
function seek(tMs: number) {
  if (!props.recording.events.length) return;
  // 二分找第一个 t_ms >= target 的事件
  let lo = 0;
  let hi = props.recording.events.length - 1;
  let target = props.recording.events.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (props.recording.events[mid].t_ms >= tMs) {
      target = mid;
      hi = mid - 1;
    } else {
      lo = mid + 1;
    }
  }
  // 翻到包含 target 的页
  page.value = Math.floor(target / PAGE);
}

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

defineExpose({ setCurrentMs: (ms: number | null) => (currentMs.value = ms) });
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
      <div style="padding: 0 22px 14px">
        <Timeline
          :events="recording.events"
          :duration-ms="recording.duration_ms"
          :current-ms="currentMs"
          @seek="seek"
        />
        <div style="color: var(--faint); font-size: 11px; margin-top: 4px; line-height: 1.5">
          点击时间轴跳转到对应步骤；颜色：<span style="color: rgba(96,165,250,0.9)">移动</span> ·
          <span style="color: rgba(239,68,68,0.9)">点击</span> ·
          <span style="color: rgba(168,85,247,0.9)">滚轮</span> ·
          <span style="color: rgba(34,197,94,0.9)">键盘</span>
        </div>
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
