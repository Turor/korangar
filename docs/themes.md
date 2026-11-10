# Interface themes

Korangar themes are small token files, not the full widget RON dump. Edit a
file, then reload — the HUD, Gear, inventory, minimap, tooltips, and world
overlay are all painted from the same tokens.

## Token file

Put one file per theme, named `themes/<id>.ron` or `themes/<id>.toml`.

```ron
ThemeFile(
    name: "Verdant Rune",
    bg: "#0F1714",
    panel: "#17221E",
    raised: "#203029",
    border: "#34483F",
    text: "#E3EDE6",
    muted: "#93A69C",
    accent: "#4CC3A0",
    accent2: "#E0708A",
    hover: "#CFAE5A",
    hp: "#D2524A",
    sp: "#4A86D8",
    gold: "#CFAE5A",
    slot_size: 44.0,
    corner_radius: 8.0,
    font_size: 13.0,
)
```

The same shape works as TOML:

```toml
name = "Verdant Rune"
bg = "#0F1714"
panel = "#17221E"
# ...
slot_size = 44.0
```

### Tokens

| Token | Role |
| --- | --- |
| `bg` | Window / chat backdrop |
| `panel` | Panel fill |
| `raised` | Slot and button fill |
| `border` | Default slot and chrome borders (**not** gold) |
| `text` | Body and title text |
| `muted` | Secondary labels |
| `accent` | Selected / dragged slots, primary highlights |
| `accent2` | Alerts, close-button accent, enemy HP |
| `hover` | Slot hover border (the theme's gold / amber) |
| `hp` | Player HP (keep warm) |
| `sp` | Player SP (keep cool) |
| `gold` | Refine, rarity, zeny, and small gold accents only |
| `slot_size` | Optional socket side in pixels (default 44) |
| `corner_radius` | Optional corner radius (default 8) |
| `font_size` | Optional body font size (default 13) |

Slot frames use `border` on a `raised` fill. Hover uses `hover` (gold/amber).
Selected or dragged slots use `accent`. Do not paint default slot borders with
`gold` or `hover`.

UI code should read `client_theme().tokens()` (or the world theme built from
the same file) instead of hard-coded colors. World-map and other new windows
should use those tokens after merge.

Unlisted tokens that you add for contrast (especially `muted`, `hp`, `sp`,
and `hover`) should keep WCAG AA body-text contrast (≥ 4.5:1) against
`panel`. Keep `hp` warm and `sp` cool; `hover` is the theme's gold or amber.

## Where files live

Search order (first match of `<id>.ron` or `<id>.toml` wins):

1. **User config** (overrides everything else)
   - Linux: `$XDG_CONFIG_HOME/korangar/themes`, defaulting to `~/.config/korangar/themes`
   - Windows: `%APPDATA%\korangar\themes`
   - Override the directory with `KORANGAR_THEMES_DIR`
2. **Bundled / install**
   - `themes/` next to the Korangar executable
   - `./themes` and `./client/themes` in the working directory
3. **Built-in** copies of Verdant Rune, Ember Forge, Arcane Violet, and
   Turoran Dark, compiled into the client if no file is found

A full widget dump in `client/in_game_themes/<id>.ron` or
`client/world_themes/<id>.ron` still loads if no token file exists, for older
custom themes.

## Bundled themes

- `verdant_rune` — default; used for first run and headless screenshots
- `ember_forge` — complementary gold on iron
- `arcane_violet` — triadic violet, gold, and teal
- `turoran_dark` — the earlier charcoal HUD (still offered in the picker)

## How to reload

- Interface Settings → **Reload theme**
- Menu → **Reload theme**
- Keybind: **F10**

The client also reloads the active theme when its file on disk changes
(mtime). Pick a theme in Interface Settings; the in-game and world theme
dropdowns list every token file that was found, plus the built-in names.

Headless / `KORANGAR_HEADLESS_AUTO` still uses Verdant Rune in memory and
does not rewrite `client/interface_settings.ron`.
