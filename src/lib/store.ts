import { create } from "zustand";
import { api, toAppError } from "./api";

export type Tab = "run" | "artists" | "history" | "rules" | "settings";
export type Theme = "system" | "light" | "dark";

interface State {
  ready: boolean;
  tab: Tab;
  settings: Record<string, string>;
  hasGemini: boolean;
  hasAnthropic: boolean;
  toast: string | null;
  /** Utwór do otwarcia w widoku „Analiza i pitch” (z Artystów / Historii). */
  openTrackId: number | null;
  openTrack: (id: number) => void;
  /** Artysta do zaznaczenia w karcie Artyści. */
  openArtistId: number | null;
  openArtist: (id: number) => void;
  setTab: (t: Tab) => void;
  notify: (msg: string) => void;
  load: () => Promise<void>;
  refreshKeys: () => Promise<void>;
  saveSetting: (key: string, value: string) => Promise<void>;
}

let toastTimer: ReturnType<typeof setTimeout> | undefined;

export function applyTheme(theme: Theme) {
  const el = document.documentElement;
  if (theme === "system") el.removeAttribute("data-theme");
  else el.setAttribute("data-theme", theme);
}

export const useApp = create<State>((set, get) => ({
  ready: false,
  tab: "run",
  settings: {},
  hasGemini: false,
  hasAnthropic: false,
  toast: null,
  openTrackId: null,
  openTrack: (id) => set({ openTrackId: id, tab: "run" }),
  openArtistId: null,
  openArtist: (id) => set({ openArtistId: id, tab: "artists" }),
  setTab: (tab) => set({ tab }),
  notify: (toast) => {
    set({ toast });
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => set({ toast: null }), 3500);
  },
  load: async () => {
    try {
      const [settings, hasGemini, hasAnthropic] = await Promise.all([
        api.getSettings(),
        api.hasSecret("gemini"),
        api.hasSecret("anthropic"),
      ]);
      applyTheme((settings.theme as Theme) || "system");
      set({ settings, hasGemini, hasAnthropic, ready: true });
    } catch (e) {
      get().notify(toAppError(e).message);
      set({ ready: true });
    }
  },
  refreshKeys: async () => {
    const [hasGemini, hasAnthropic] = await Promise.all([
      api.hasSecret("gemini"),
      api.hasSecret("anthropic"),
    ]);
    set({ hasGemini, hasAnthropic });
  },
  saveSetting: async (key, value) => {
    await api.setSetting(key, value);
    set((s) => ({ settings: { ...s.settings, [key]: value } }));
    if (key === "theme") applyTheme(value as Theme);
  },
}));
