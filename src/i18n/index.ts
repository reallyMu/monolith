import { computed, ref } from "vue";
import { messages, type Locale, type MessageKey } from "./messages";

export type { Locale, MessageKey };

export function detectLocale(): Locale {
  const lang =
    (typeof navigator !== "undefined" && (navigator.language || navigator.languages?.[0])) || "en";
  return lang.toLowerCase().startsWith("zh") ? "zh" : "en";
}

const localeRef = ref<Locale>(detectLocale());

export function getLocale(): Locale {
  return localeRef.value;
}

export function setLocale(locale: Locale) {
  localeRef.value = locale;
  if (typeof document !== "undefined") {
    document.documentElement.lang = locale === "zh" ? "zh-CN" : "en";
  }
}

export function t(key: MessageKey, vars?: Record<string, string | number>): string {
  const table = messages[localeRef.value] ?? messages.en;
  let s = table[key] ?? messages.en[key] ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      s = s.split(`{${k}}`).join(String(v));
    }
  }
  return s;
}

/** Reactive helper for templates. */
export function useI18n() {
  const locale = computed(() => localeRef.value);
  return {
    locale,
    t: (key: MessageKey, vars?: Record<string, string | number>) => {
      // depend on locale so template re-renders
      void locale.value;
      return t(key, vars);
    },
  };
}

setLocale(detectLocale());
