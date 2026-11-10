# Headless login

`KORANGAR_HEADLESS_AUTO=1` logs in with the remembered service credentials, clicks the first character server immediately, enters character slot 0, and opens Gear once the map is ready.

Screenshot runs load the built-in **Verdant Rune** in-game and world themes (`verdant_rune`) even when `client/interface_settings.ron` names another theme. The saved file is left unchanged. A fresh install with no settings file also starts on Verdant Rune. Turoran dark stays selectable as `turoran_dark`.

It does not open the friend list, standalone Stats, Inventory, Character Overview, Create Character, or the character menu. Inventory stays on Alt+E. Gear stays on Alt+Q. A map session does not open Create Character; that window belongs to character select.

The Select Server choice is cloned before the character-server list is written into client state, so the click is not lost to a deferred state update. The path is meant to reach a map session inside the Hercules login `AUTH_TIMEOUT` of 30 seconds. Do not raise that timeout.
