# Headless login

`KORANGAR_HEADLESS_AUTO=1` logs in with the remembered service credentials, clicks the first character server immediately, enters character slot 0, and opens Gear once the map is ready.

It does not open the friend list, standalone Stats, or Inventory. Inventory stays on Alt+E. Gear stays on Alt+Q.

The Select Server choice is cloned before the character-server list is written into client state, so the click is not lost to a deferred state update. The path is meant to reach a map session inside the Hercules login `AUTH_TIMEOUT` of 30 seconds. Do not raise that timeout.
