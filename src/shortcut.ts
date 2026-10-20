// Mirrors `config::ShortcutConfig`; shared by Settings and the widget.
export type ShortcutConfig = {
  meta: boolean;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  code: string;
};

export function prettyCode(code: string): string {
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  return code;
}

/// Mirrors `shortcut::skip_variant`: the hotkey plus Shift (plus Option if
/// it already has Shift) is "paste as-is".
export function skipVariant(cfg: ShortcutConfig): ShortcutConfig {
  return cfg.shift ? { ...cfg, alt: true } : { ...cfg, shift: true };
}

export function shortcutLabel(cfg: ShortcutConfig): string {
  let s = "";
  if (cfg.ctrl) s += "⌃";
  if (cfg.alt) s += "⌥";
  if (cfg.shift) s += "⇧";
  if (cfg.meta) s += "⌘";
  return s + prettyCode(cfg.code);
}
