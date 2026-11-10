# Item Finder exporters

These scripts rebuild the two JSON files the client loads from the `archive/` overlay:

- `korangar/archive/System/itemfinder.turoran.v8.json` (catalog; generator field `v` stays 7, matching the published schema)
- `korangar/archive/System/itemsources.turoran.v3.json` (`v`: 3; drops, mobs, quests, penalty, rates)

`genitemfinder.py` and `genitemsources.py` are adapted from the Turoran generators (`lcfg.py` plus the item/job helpers in `geniteminfo2.py`). `item_db2.conf` is optional.

## Re-run against a Hercules root

From the repository root, with a Hercules checkout that contains `db/re/item_db.conf`, `db/re/mob_db.conf`, `db/re/level_penalty.txt`, and `conf/map/battle/drops.conf`:

```bash
python3 scripts/itemfinder/export.py \
  /path/to/hercules \
  /path/to/itemInfo.turoran.renewal11.lua \
  korangar/archive/System/itemfinder.turoran.v8.json \
  korangar/archive/System/itemsources.turoran.v3.json
```

Optional `db/item_db2.conf` is picked up when it exists. Pass a client `mapnametable.txt` by replacing the `"-"` map argument inside `genitemsources.py` if you call that script directly:

```bash
python3 scripts/itemfinder/genitemsources.py \
  /path/to/hercules \
  /path/to/mapnametable.txt \
  korangar/archive/System/itemsources.turoran.v3.json
```

Quest file lists and mob spawn rows come from NPC scripts loaded by `npc/re/scripts_main.conf` (the same walk the original generator uses). A full hercules-dev checkout is required for that pass. `mapnametable.txt` supplies the display names stored in each mob `sp` row.

## This tree (CT119 snapshot, no NPC scripts)

`/root/Work/korangar-itemfinder-inputs` has `db/re`, drop rates, and `itemInfo`, and an empty `npc/` directory. Quest scripts are not on this host. Re-run drops, mob stats, penalty, and rates from that snapshot, and keep quest lists plus spawn rows from `itemsources.turoran.v2.json`:

```bash
python3 scripts/itemfinder/export.py \
  /root/Work/korangar-itemfinder-inputs \
  /root/Work/korangar-itemfinder-inputs/itemInfo.turoran.renewal11.lua \
  korangar/archive/System/itemfinder.turoran.v8.json \
  korangar/archive/System/itemsources.turoran.v3.json \
  /root/Work/korangar-itemfinder-inputs/itemsources.turoran.v2.json
```

When `npc/re/scripts_main.conf` exists, the fourth fallback file is ignored and quests and spawns are parsed from the NPC tree again.

The client reads these paths through the `archive/` folder overlay (`GameArchiveList::LOCAL_OVERLAY`). The working directory is adjusted to `korangar/` when `archive/` is not already the current directory.
