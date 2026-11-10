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
| `KORANGAR_AUTOPILOT_CMDS` | Fifo or file of line commands, read after the map loads, or earlier while a PIN holds character select. |
| `KORANGAR_AUTOPILOT_PIN` | PIN the pad enters and submits by itself when the character server asks for one. Never logged. |

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
| `open <WindowClass>` / `close <WindowClass>` | Open or close that window. Names match `WindowClass` (`Inventory`, `Equipment`, `Menu`, `WorldMap`, …). `gear` is `Equipment`. |
| `toggle <Event>` | Unit `InputEvent` whose name starts with `Toggle` or `Open`, for example `toggle ToggleInventoryWindow` or `toggle ToggleWorldMap`. |
| `event <spec>` | A unit `InputEvent` name, or one parameterized form below. |
| `wait <ms>` | Pause the command reader. |
| `move <x> <y>` | Walk to that tile. Walks longer than Hercules accepts in one request (14 cells) are split into segments, like a mouse click. |
| `click <x> <y>` / `rclick` / `dclick` | Pointer event in interface pixels, fed through the normal input report. |
| `drag <x0> <y0> <x1> <y1>` | Press, move, and release across the next frames. Interface pixels. |
| `key <KeyCode>` / `key alt+KeyE` | Physical key. Names are `winit` `KeyCode` variants. Modifiers: `alt`, `ctrl`, `shift`. |
| `pincode digit <0-9>` | Press the shuffled pad key that shows that digit. |
| `pincode press <0-9>` | Press one pad location (the label there varies). |
| `pincode submit` / `change` / `reset` / `cancel` / `request` | The pad buttons: submit the entry, start a PIN change, clear the entry, leave to login, or ask for the pad again. |
| `dump <path>` | Print `AUTOPILOT_DUMP <path> …`. Paths: `inventory`, `hotbar`, `chat_messages`, `windows`, `player`. |
| `quit` | Ask the client to exit. |

`chat @where <name>` reports a tile (`Server: <name> <map> <x> <y>`). A bare `@where` fails with `Please enter a player name`.

A left `click` on an NPC sprite box walks within three cells and then opens its dialog.

`InputEvent` does not implement serde. `event` therefore uses a curated table:

- Every unit variant, by Rust name (`ToggleInventoryWindow`, `OpenBank`, `Respawn`, …).
- `SendMessage("text")`
- `PlayerMove(x: 10, y: 20)`
- `SelectCharacter(0)`
- `CreateCharacter(slot: 0, name: "Name")` — optional `hair_color` (default 3), `hair_style` (default 2), `start_job` (default 0, Novice), `sex` (default 1, male). Empty-slot login uses those defaults.
- `OpenCharacterCreationWindow(0)`
- `CastSkill(0)` / `StopSkill(0)` (hotbar slot)
- `UseItem(0)` (inventory index)
- `SelectItemSkillRow(0)`
- `ConfirmItemSkillSelection` / `CancelItemSkillSelection`
- `ZoomCamera(1.0)` / `RotateCamera(0.1)`
- `SendEmotion(1)`
- `StatUpStrength(1)`, `StatUpAgility(1)`, `StatUpVitality(1)`, `StatUpIntelligence(1)`, `StatUpDexterity(1)`, `StatUpLuck(1)`
- `RequestStableHomunculus(homunculus_id: 0)` (id 0 stores the active homunculus)
- `RequestStablePet(action: 0, id: 0)` (`action` is the stable pet word; id 0 stores the current pet)
- `CreateParty(name: "Name", item_pickup: false, item_division: false)`
- `InviteToParty("Character")`
- `OpenPlayerContextMenu(2000043)` (a player entity id, which is its account id)
- `ContextMenuPick(1)` (a row in the open player context menu)
- `TradeAddItem(index: 4, amount: 1)`, `TradeAddZeny(100)`
- `SetShowEquipment(true)`
- `LeaveParty` and `AcceptPendingPartyInvite` (unit events; accept uses the open invite)

`event LogIn` is rejected so a command file cannot be asked to print a password.

## PIN

When the character server asks for a PIN (`ZC_SECOND_PASSWD_LOGIN` states 1, 2, 3, 4 and 8), the pad opens by itself on top of character select. It reopens if character select is rebuilt after the request, and closes when the server accepts the PIN (state 0). State 7 only shows the PIN button.

If `KORANGAR_AUTOPILOT_PIN` is set, the client types and submits that PIN, and the pad stays closed. Otherwise, while select is held the client prints `AUTOPILOT_PIN_GATE` and reads line commands before the map loads, so a script can drive the pad:

```text
pincode digit 1
pincode digit 2
pincode digit 3
pincode digit 4
pincode submit
```

A login-server failure whose message mentions “pin” moves the autopilot to an awaiting-pin state. With `KORANGAR_AUTOPILOT_PIN` set it prints `AUTOPILOT_PIN_HOOK armed`. The code is never logged.
