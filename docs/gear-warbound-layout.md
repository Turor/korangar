# Gear layout

Dark Warbound-style framing. Three columns: square sockets on the left, one tall portrait filling the center column, square sockets on the right. Stats are a compact block on the right half of Gear. Inventory is its own window. Alt+E opens Inventory alone.

```
+--------------------------------------------------+
| Gear                                             |
+---------------------------+----------------------+
| Equipment                 | Base              12 |
| [H.Top] [            ] [H.Mid]                   |
| [H.Low] [            ] [Armor]  STR 40 +2 [+]    |
| [R.Hnd] [  portrait  ] [L.Hnd]  AGI 30 +0 [+]    |
| [Garm]  [            ] [Shoes]  VIT / INT        |
| [R.Acc] [            ] [L.Acc]  DEX / LUK        |
|         [   Ammo   ]                             |
| Costume                   | Combat               |
| [C.Top][C.Mid][C.Low][C.Garm]  ATK  120 + 45     |
| Shadow                    | MATK  20 +  8        |
| [S.Weapon] ... [S.Acc R]  | DEF MDEF HIT FLEE    |
| Sets                      | CRIT ASPD            |
| [name________]                                   |
| [Save] [Load] [Rename]                           |
| [Town] [Hunt]  (one click)                       |
+---------------------------+----------------------+
```

Every gear, costume, shadow, and ammo socket is the same square (`GEAR_SLOT_SIDE` by `GEAR_SLOT_SIDE`). A short caption sits under the square. The full name is the tooltip. The portrait is one framed column from the head-top row through the accessory row. Ammo is centered under the portrait.

Base stats pack name, base, bonus, and the point button into one cluster. Combat stats pack the name and the base + bonus the same way. The cluster does not stretch across the right half. Base Lv and Job Lv stay on the character overview.

First open, with no saved window position, scales from 1280×720. Gear is the upper left. Inventory is to its right. Stats are placed inside Gear's right half. Chat is the bottom left. The hotbar is bottom center. The minimap is the top right, with status under it. Quest, friends, party, and guild stack on the right, clear of Gear. The skill tree is centered. Those rectangles stay on screen at 1280×720 and 1920×1080. Gear, Inventory, chat, hotbar, minimap, status, and the four social windows do not overlap. The skill tree is centered and can cover other windows while it is open.

The portrait draws the controlled player's idle sprite. That sprite is left out of the world pass while Gear is open.

Loadouts snapshot the server switch set (`EquipSwitch`). Save stores the live set under the draft name. Rename edits the selected set. Load writes that snapshot back with the switch add and remove packets. A set name wears it in one click (write, then activate).
