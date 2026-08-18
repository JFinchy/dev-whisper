export type ThemeId = "terminal" | "signal" | "quiet" | "palette";

export type ThemeTokens = {
  bg: string;
  fg: string;
  muted: string;
  border: string;
  card: string;
  accent: string;
  accentFg: string;
  navActiveBg: string;
};

// Color-only skins for the signal-chain settings layout — see
// manual-testing-inbox/ and the design-review artifact this was built
// from. Terminal is the default: this is a local developer tool first,
// not a consumer dictation app, and its dark/amber identity says that on
// sight.
export const THEMES: Record<ThemeId, ThemeTokens> = {
  terminal: {
    bg: "#16181B",
    fg: "#E9E7E1",
    muted: "#8A8F98",
    border: "#2A2D32",
    card: "#1C1F23",
    accent: "#E8A33D",
    accentFg: "#16181B",
    navActiveBg: "#262115",
  },
  signal: {
    bg: "#F5F1E9",
    fg: "#22201B",
    muted: "#83786A",
    border: "#E4DBC9",
    card: "#FBF8F2",
    accent: "#1F6F63",
    accentFg: "#F5F1E9",
    navActiveBg: "#E1EFEA",
  },
  quiet: {
    bg: "#FFFFFF",
    fg: "#1C1C1E",
    muted: "#75767A",
    border: "#E6E6E3",
    card: "#FBFBFA",
    accent: "#3B6FD9",
    accentFg: "#FFFFFF",
    navActiveBg: "#EAF0FD",
  },
  palette: {
    bg: "#0E0F12",
    fg: "#F2F2F0",
    muted: "#83858C",
    border: "#232429",
    card: "#191A1E",
    accent: "#FF6B4A",
    accentFg: "#0E0F12",
    navActiveBg: "#241A16",
  },
};

export const THEME_ORDER: ThemeId[] = ["terminal", "signal", "quiet", "palette"];

export const THEME_LABEL: Record<ThemeId, string> = {
  terminal: "Terminal — dark, monospace-forward",
  signal: "Signal — warm paper, teal accent",
  quiet: "Quiet — light, minimal",
  palette: "Palette — near-black, coral accent",
};

const VAR_MAP: Record<keyof ThemeTokens, string> = {
  bg: "--dw-bg",
  fg: "--dw-fg",
  muted: "--dw-muted",
  border: "--dw-border",
  card: "--dw-card",
  accent: "--dw-accent",
  accentFg: "--dw-accent-fg",
  navActiveBg: "--dw-nav-active-bg",
};

// Applies a theme's tokens as CSS custom properties on the document root
// so both plain CSS (`var(--dw-*)`) and inline styles can read them —
// used by both SettingsView (full theme) and WidgetView (accent only).
export function applyTheme(id: ThemeId) {
  const tokens = THEMES[id];
  const root = document.documentElement.style;
  (Object.keys(VAR_MAP) as (keyof ThemeTokens)[]).forEach((key) => {
    root.setProperty(VAR_MAP[key], tokens[key]);
  });
}
