# Headless autopilot

The client stays a normal player build. Nothing in this path runs unless `KORANGAR_AUTOPILOT` is set to an account JSON file:

```json
{"username": "example", "password": "…", "login_address": "192.168.11.110:6910"}
```

The password is never printed. Do not commit account files.

## Login

The client pushes `InputEvent::LogIn`, `SelectServer`, and `SelectCharacter` directly. It does not click the mouse.

| Variable | Meaning |
| --- | --- |
| `KORANGAR_AUTOPILOT` | Path to the account JSON. Unset means autopilot is off. |
| `KORANGAR_AUTOPILOT_SERVER` | Character-server name. Empty uses the first server. |
| `KORANGAR_AUTOPILOT_SLOT` | Character slot. Default `0`. |
| `KORANGAR_AUTOPILOT_CHAR` | Name used when that slot is empty. Default is the username with non-alphanumerics removed. |
| `KORANGAR_AUTOPILOT_REGISTER` | `1` logs in as `<username>_M` so Hercules `new_account` creates the account. Later sessions must leave this unset. |
| `KORANGAR_AUTOPILOT_CMDS` | Fifo or file of line commands, read after the map loads. |
| `KORANGAR_AUTOPILOT_PIN` | Held for a future PIN prompt. Never logged. G02 reads it with `Autopilot::take_pending_pin`. |

When the window is shown the client records a clock. After the in-world map loads it prints:

```text
AUTOPILOT_TIMING login_ok=…ms server_selected=…ms char_selected=…ms map_loaded=…ms
AUTOPILOT_READY
```

Times are milliseconds from the window being shown. `map_loaded` has to stay under the login server `AUTH_TIMEOUT` of 30000 ms. Do not raise that timeout.

## Commands

Each line is logged as `AUTOPILOT_CMD ok|err <line>`. Blank lines and `#` comments are ignored.

| Command | Effect |
| --- | --- |
| `chat <text>` | `SendMessage` (including `@` GM commands). |
| `open <WindowClass>` / `close <WindowClass>` | Open or close that window. Names match `WindowClass` (`Inventory`, `Equipment`, `Menu`, …). `gear` is `Equipment`. |
| `toggle <Event>` | Unit `InputEvent` whose name starts with `Toggle` or `Open`, for example `toggle ToggleInventoryWindow`. |
| `event <spec>` | A unit `InputEvent` name, or one parameterized form below. |
| `wait <ms>` | Pause the command reader. |
| `move <x> <y>` | Walk to that tile. |
| `click <x> <y>` / `rclick` / `dclick` | Pointer event in interface pixels, fed through the normal input report. |
| `key <KeyCode>` / `key alt+KeyE` | Physical key. Names are `winit` `KeyCode` variants. Modifiers: `alt`, `ctrl`, `shift`. |
| `dump <path>` | Print `AUTOPILOT_DUMP <path> …`. Paths: `inventory`, `hotbar`, `chat_messages`, `windows`, `player`. |
| `quit` | Ask the client to exit. |

`InputEvent` does not implement serde. `event` therefore uses a curated table:

- Every unit variant, by Rust name (`ToggleInventoryWindow`, `OpenBank`, `Respawn`, …).
- `SendMessage("text")`
- `PlayerMove(x: 10, y: 20)`
- `SelectCharacter(0)`
- `CreateCharacter(slot: 0, name: "Name")`
- `OpenCharacterCreationWindow(0)`
- `CastSkill(0)` / `StopSkill(0)` (hotbar slot)
- `ZoomCamera(1.0)` / `RotateCamera(0.1)`
- `SendEmotion(1)`
- `StatUpStrength(1)`, `StatUpAgility(1)`, `StatUpVitality(1)`, `StatUpIntelligence(1)`, `StatUpDexterity(1)`, `StatUpLuck(1)`
- `RequestStableHomunculus(homunculus_id: 0)` (id 0 stores the active homunculus)
- `RequestStablePet(action: 0, id: 0)` (`action` is the stable pet word; id 0 stores the current pet)

`event LogIn` is rejected so a command file cannot be asked to print a password.

## PIN hook

A login failure whose message mentions “pin” moves the autopilot to an awaiting-pin state. If `KORANGAR_AUTOPILOT_PIN` is set the client prints `AUTOPILOT_PIN_HOOK armed` and keeps the code in memory. It does not send a PIN packet. G02 should call `Autopilot::take_pending_pin`.
