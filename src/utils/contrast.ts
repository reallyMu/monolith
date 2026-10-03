/** Relative luminance (sRGB) 0–1. */
export function luminance(hex: string): number {
  const [r, g, b] = hexToRgb(hex);
  const f = (c: number) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

export function isDarkBg(hex: string): boolean {
  return luminance(hex) < 0.45;
}

/** Primary text that contrasts with background. */
export function contrastText(bg: string): string {
  return isDarkBg(bg) ? "#e6e8ee" : "#1c2028";
}

/** Muted secondary text. */
export function contrastMuted(bg: string): string {
  return isDarkBg(bg) ? "#9aa3b2" : "#5a6474";
}

/** Monaco theme colors must be #RRGGBB or #RRGGBBAA (not rgba()). */
export function contrastHighlightBg(_bg: string): string {
  // transparent fill — we use a border box instead
  return "#00000000";
}

/** Soft outline for current / synced line (hex+alpha). */
export function contrastHighlightBorder(bg: string): string {
  return isDarkBg(bg) ? "#8aa4c499" : "#4a6a8ecc";
}

/** Highlighted line text — slight emphasis vs body. */
export function contrastHighlightFg(bg: string): string {
  return isDarkBg(bg) ? "#f2f5fa" : "#0f141c";
}

/** Soft gutter tick for current line. */
export function contrastHighlightGutter(bg: string): string {
  return isDarkBg(bg) ? "#8aa4c4aa" : "#4a6a8e99";
}

/** Mix hex with white/black for borders. */
export function contrastBorder(bg: string): string {
  return isDarkBg(bg) ? "rgba(255,255,255,0.10)" : "rgba(0,0,0,0.12)";
}

/** Selection wash (hex+alpha). */
export function contrastSelection(bg: string): string {
  return isDarkBg(bg) ? "#6a8ab048" : "#4a6a8e40";
}

export function hexToRgb(hex: string): [number, number, number] {
  let h = hex.trim().replace("#", "");
  if (h.length === 3) h = h.split("").map((c) => c + c).join("");
  if (h.length !== 6) return [26, 29, 35];
  const n = Number.parseInt(h, 16);
  if (Number.isNaN(n)) return [26, 29, 35];
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}
