# Styling guide

Fonts, colors, chat-channel colors, slot and item-box shadows, window frame
outlines, and overhead health-bar colors come from theme tokens. Do not add a
new literal color or a one-off `FontSize(16.0)` on a surface you are already
editing.

Token files live in `themes/<id>.ron` (see `docs/themes.md`). `ThemeFile` is
expanded into widget themes by `InterfaceTheme::from_tokens` and
`WorldTheme::from_tokens`.

## Fonts

The theme `font_size` (13 by default) is the body size at 100%. The player
font scale is Settings → Interface, **Font scale %**, range 80–150. It is
stored on `interface_settings.ron` as `font_scale` and applied in two places:

- Chrome row heights (window title, buttons, text rows, fields, drop-downs)
  are multiplied once when the theme is loaded.
- UI text measurement multiplies the same factor in the text layouter. The
  interface renderer applies it once more when drawing glyphs. World and
  damage text do not use this scale.

A size that used to be a literal pixel value should be
`theme font_size * historical_px / 13`, then let measurement apply the player
scale. `FontSize(tokens.font_size)` already tracks the token.

## Colors

Read `client_theme().tokens()` (`bg`, `panel`, `raised`, `border`, `text`,
`muted`, `accent`, `accent2`, `hover`, `hp`, `sp`, `gold`). Slot borders use
`border`. Gold and `hover` are for hover, selection, and small accents only.

## Chat channels

`chat.party`, `chat.broadcast`, `chat.server`, `chat.error`,
`chat.information`, `chat.public`, `chat.whisper`, and `chat.guild` are
optional `#RRGGBB` strings. Omitted keys are derived from the palette. Party
defaults to `#E6D7C8` when the file does not set it. `ChatTheme` copies these
tokens; do not hardcode a channel color in a window.

## Slots and item boxes

`slot_shadow` is optional. When the file omits it, the shadow is `bg` at the
historical shadow alpha. `paint_item_slot` reads that token. Keep the existing
shadow padding.

## Window frames

Every window, including the minimap, menu bar, chat, context menu, and the
NPC dialog band, draws a frame from `window.frame_color` (the theme `border`
token) and `window.frame_width`. Do not pass a one-off outline color into
`window!`.

Tooltip and context-menu backgrounds use the same `window_alpha` as window
panels. At the default `window_alpha` of 1 they are fully opaque.

## Overhead HP

Overhead health colors are world-theme tokens: the player bar is
`status.hp_bar` and the enemy bar is `danger` (`status.danger` is the other
spelling). Change those keys in the theme file. Do not paint a new literal
on the world bar.
