# Insights page

Branch: `main` (`f634fcc`, 2026-08-18; never queued for a manual pass)
Files changed: `src-tauri/src/insights.rs` (new), `src-tauri/src/lib.rs`,
`src/SettingsView.tsx`

## What to verify

Settings → Insights shows totals, words per minute, per-app usage, a
streak, and a feature-adoption checklist, all computed from History.
Unit tests cover the math; nothing has checked it renders sensibly
against real history.

## Steps

1. With some History present, open Settings → Insights. Totals and the
   per-app breakdown roughly match what you've dictated (Orca, Notes...).
2. Words per minute and personal best look plausible (roughly 100–180),
   not 0, NaN or absurd.
3. Dictate one snippet or "…, actually …" correction, reopen Insights:
   the matching adoption checklist item flips to done.

## Pass criteria

Numbers are plausible and update after new dictations.
