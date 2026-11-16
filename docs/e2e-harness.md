# In-client end-to-end harness

Scriptable input, a no-swapchain logic mode, and `cargo test -p korangar --test e2e`. This is the in-repo piece of the headless autopilot (`docs/autopilot.md`) and of `$HUB/verify/run-session.sh`. It does not replace that Xvfb session runner.

The harness does not touch theme tokens, Settings → Interface controls, or the NPC dialog window layout. G42 owns theming. G44 owns dialog presentation. Scenarios open the existing Settings group and choose a dialog option by index or label.

## Input injection

`korangar::e2e` drives the same `InputSystem` the winit frame uses, in the same order as `Client::update_and_render`:

1. `expire_synthesized`
2. `advance_synthetic_drag`
3. The scripted press, click, drag, or `buffer_character` (the call the winit text path makes)
4. `update_delta`
5. `handle_keyboard_input`

Keyboard, mouse, and drag scripts call `synthesize_key`, `synthesize_click`, and `begin_synthetic_drag`. Those are the autopilot methods. Window and dialog actions reuse the autopilot line language (`open`, `tab`, `event`, `click`, `key`, …) via `parse_command`, then apply the same `InputEvent` variants the client match arms apply (`ToggleGameSettingsWindow`, `SwitchWindowTab`, `ChooseDialogOption`, …).

Dialog choices use the public option index. The wire value is `index + 1`, the same formula as the dialog button. Scenarios do not click sprite pixels.

An offline test runs one winit-shaped frame and one injected frame and checks that both produce `InputEvent::Escape`, the same text buffer, a left click, and a drag. No server is involved.

## Run modes

| Mode | When | What it can assert |
| --- | --- | --- |
| Logic | Default for `cargo test`. No swapchain and no window. | Input reports, the open Settings member and that member's tabs, dialog text / options / selected index, outbound packet intents. Live tests also send those packets through the same calls the client match arms use. |
| Lavapipe probe | `lvp_icd.json` is installed. `capture_lavapipe` asks wgpu for the fallback Vulkan adapter and reads a 64×64 cleared offscreen target back to `lavapipe-probe.png`. | That the software adapter can produce an image. The PNG is a solid clear color. It is not the Settings window and is not named `settings-group.png`. A missing ICD skips. |
| Xvfb + lavapipe client | `KORANGAR_E2E=1`. `capture_client_settings` runs `$HUB/verify/run-session.sh`, which logs in, applies `open GameSettings` and `tab AudioSettings`, then ImageMagick `import -window` of the Korangar window. | `settings-group.png` is that client window (at least 640×480, not a single color). |

Logic mode cannot check screenshots, GPU picking, or shader output. A click in logic mode still goes through `InputSystem`, but there is no picker target, so talking to an NPC passes the entity id into the same `start_dialog` call the in-world `TalkToNpc` action uses.

Set `VK_ICD_FILENAMES` to the lavapipe ICD when you want the full client on the software GPU. The verify script already does that under Xvfb. Do not point the client at a live login server.

## Credentials

Live scenarios connect only when `KORANGAR_E2E=1`. `KORANGAR_AUTOPILOT` and `KORANGAR_E2E_ACCOUNT` only choose the account file. A shell that already uses autopilot does not dial TEST from `cargo test`.

The file is the autopilot JSON: `username`, `password`, `login_address`. With `KORANGAR_E2E=1` and no path, the harness tries `KORANGAR_E2E_ACCOUNT`, then `KORANGAR_AUTOPILOT`, then the verify pool file `kgap29.json`, then the private m1 account file, if they exist on the machine. Nothing in the repo contains a password.

`login_address` must be `192.168.11.110:6910`. `Credentials::resolve` refuses any other address before it opens a socket. The character and map servers must also be that host. Passwords are never written to logs, assertions, or `Debug`. If the opt-in, the account file, or TEST is missing, the live tests print `E2E_SKIP` and return. They do not fail, so `cargo test --workspace` stays green.

## Cargo entry point

```bash
cargo test -p korangar --test e2e -- --test-threads=1
```

`$HUB/local-merge-gate.sh` runs that command when `korangar/tests/e2e.rs` exists. It does not set `KORANGAR_E2E`, so the gate's live scenarios skip. Offline cases in the same binary always run.

The three live scenarios, when `KORANGAR_E2E=1`, an account file, and TEST are available:

1. Log in and assert `Phase::CharacterSelect` after the server `CharacterList`. The harness does not invent a `CharacterSelection` window.
2. Enter the map, open Game settings, and switch to Audio with `InputEvent::SwitchWindowTab`. Assert the open member is Audio settings, its group is Settings, and that group's tabs are Interface, Graphics, Audio, Game, and Keys.
3. Talk to an NPC and select a menu option by index. Assert the dialog text, the selected index, and the serialized `ChooseDialogOption` (`0x00B8`) npc id and choice byte.

`Command::Move` and `Command::Chat` queue `PlayerMove` and `SendMessage` and send through the same networking calls as those match arms. A Next page replaces the dialog text, matching `Dialog::add_text`.

Screenshots go to `KORANGAR_E2E_SHOT_DIR` when that variable is set (the gap runs use `/root/Work/korangar-screens/gap/G46`). They are not committed. Logic mode skips image assertions. The settings logic scenario does not fail when the lavapipe probe cannot run. The client-window shot runs only with `KORANGAR_E2E=1` and uses a different pool account than the logic session.
