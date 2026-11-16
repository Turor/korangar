# Local GRF and folder overlays

Operators choose local archives with one file, `client/game_archives.ron`.
There is no settings screen for this. The old list still loads:

```ron
(archives: ["translation.grf", "data.grf", "archive/"])
```

In that list the last entry wins, same as before. That is the loader form of
DATA.INI: slot 0 is the highest configured archive, and it is stored at the
end of `archives`.

Optional fields add overlays and per-entry tolerance for a missing path:

```ron
(
    overlays: [
        "overlay/",
        (path: "/opt/korangar/patch.grf", optional: false),
    ],
    archives: [
        "data.grf",
        (path: "optional-translation.grf", optional: true),
    ],
)
```

`overlays` are folders or GRF/7z files searched before everything else. The
first overlay is the highest priority. A bare string is required (`optional:
false`). `optional: true` skips a missing path with no log line.

## Paths

Relative paths resolve against the install directory, which is the directory
of the executable. Cargo dev and test binaries resolve against the working
directory instead. Startup prints one line naming which directory it used.
Absolute paths are kept as written. `~` is not expanded to the home directory.
`http://`, `https://`, and `file://` URLs are rejected. The client does not
download or pack GRFs.

When `client/game_archives.ron` is missing, an explicit `DATA.INI` is used.
Implicit default GRF names that are not on disk are omitted. A DATA.INI row
that names a missing file logs a warning.

## Search order

Highest priority first:

1. Overlays, in listed order.
2. Configured archives (`archives` or DATA.INI). The last `archives` entry wins.
3. Korangar's `archive/` directory, when it exists and is not already listed.
4. The verified remote cache.
5. A remote fetch for a path that is not cached yet.

Remote mode still mounts overlays and configured GRFs when the config file
exists. A missing required entry logs one warning that names the resolved
path. Startup continues when another archive, `archive/`, or remote mode is
present. An optional miss is silent. If the config was read but nothing
could be mounted and remote mode is off, startup still stops with
"No game assets configured".

The derived `cache.7z` key includes each mounted archive's role, resolved
path, and bytes, plus the remote snapshot revision. A different overlay set
does not reuse that cache. A cache whose stored hash does not match is left
unopened.

Enchant tables may still come from the remote client. A local overlay can
replace any file, including `.lub`.
