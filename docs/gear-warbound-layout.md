# Gear layout

Dark Warbound-style framing. The character is not a centered-only paperdoll. Slots ring a preview cell. Stats fill the right half. Inventory is a third column so it stays reachable without a second window.

```
+------------------------------------------------------------------+
| Gear                                                             |
+---------------------------+------------------+-------------------+
| Equipment                 | Base          12 | Inventory         |
| [Head top][mid][low]      | STR  40 +2  [+]  | [ ][ ][ ][ ]...   |
| [Weapon] [Preview] [Shield| AGI  30 +0  [+]  | [ ][ ][ ][ ]...   |
| [Acc]    [Armor]   [Acc]  | VIT  ...         |                   |
| [Garment][Shoes]  [Ammo]  | INT / DEX / LUK  |                   |
| Costume                   | Combat           |                   |
| [cTop][cMid][cLow][cGarm] | ATK   120 + 45   |                   |
| Shadow                    | MATK   20 +  8   |                   |
| [..][..][..]              | DEF / MDEF       |                   |
| Sets                      | FLEE / HIT       |                   |
| [name________]            | CRIT / ASPD      |                   |
| [Save] [Load] [Rename]    |                  |                   |
| [Town] [Hunt]  (one click)|                  |                   |
+---------------------------+------------------+-------------------+

Screen 1280x720, first open (no saved window positions):

  Gear   top-left     16,36    1040x500
  Chat   bottom-left   8,552    340x160
  Hotbar bottom-center 430,664  420x48

Those three rectangles do not overlap. Base Lv and Job Lv stay on the
character overview, not in Gear.
```

Loadouts snapshot the server switch set (`EquipSwitch`). Save stores the live set under the draft name. Rename edits the selected set. Load writes that snapshot back with the switch add and remove packets. A set name wears it in one click (write, then activate).
