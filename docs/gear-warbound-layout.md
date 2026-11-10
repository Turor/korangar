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
| [S.Wep] ... [S.Sho]       | DEF MDEF HIT FLEE    |
| [S.L] [S.R] [name____] + -| CRIT ASPD            |
| [Save] [Load] [Rename]                           |
| [Town] [Hunt]  (one click)                       |
+---------------------------+----------------------+
```

Every gear, costume, shadow, and ammo socket is the same 44px square at every client size. The first-open Gear window is that reference size (it is not scaled down at 1024×576). A 24px item icon is centered in the frame and is not stretched. Empty sockets keep a visible dark border. A short caption sits in the bottom of the square, clipped to that column, so neighbors do not overlap. Shadow accessory captions are `S.L` and `S.R`. Shadow wraps to a second row. The full name is the tooltip. The portrait is one framed column from the head-top row through the accessory row. Ammo is a square inside that frame, over the portrait just below the character's feet.

Sets are one row: a dropdown of saved names, then + and −. Together + and − are half the dropdown width. + stores the current switch set as `Set N` (or the typed name). Choosing a name loads that set through SwitchEquip. − deletes the selected set and does nothing when none is selected. Double-click is the inline rename path (`BeginRenameLoadout`).

Base stats pack name, base, bonus, and the point button into one cluster. Combat stats pack the name and the base + bonus the same way. The cluster does not stretch across the right half. Base Lv and Job Lv stay on the character overview.

First open, with no saved window position, scales other HUD frames from 1280×720. Gear stays the reference pixel size so sockets stay 44px. Gear is the upper left. Inventory is to its right. The character menu and standalone Stats stack under Inventory, clear of Gear, because Gear already shows stats. Chat is the bottom left, below Gear. The hotbar is bottom center. The minimap is the top right, with status under it. Quest, friends, party, and guild stack below Stats, clear of Gear. The skill tree is centered. Those rectangles stay on screen at 1024×576, 1280×720, and 1920×1080 and do not overlap each other. The skill tree is centered and can cover other windows while it is open.

Combat ASPD is Hercules amotion converted the way roBrowser `WinStats` does: `floor((2000 - amotion) / 10)`, so 550 is shown as 145. HIT, FLEE, CRIT, ATK, MATK, DEF, and MDEF stay the packet numbers. roBrowser does not divide CRIT.

The portrait draws the controlled player's idle sprite. That sprite is left out of the world pass while Gear is open.

Loadouts snapshot the server switch set (`EquipSwitch`). Save stores the live set under the draft name. Rename edits the selected set. Load writes that snapshot back with the switch add and remove packets. A set name wears it in one click (write, then activate).
