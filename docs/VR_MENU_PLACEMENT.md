## Fixed Escape menu surface

Escape captures a level menu surface once, half a metre ahead of the untracked
seated camera. Head motion and submenus retain its world pose. Closing/reopening
or recentering resets it; resizing preserves centre and axes while adjusting
aspect and cursor mapping. In unpaused multiplayer the bus can move away from
this world-fixed surface; reopen it to reposition. Headset-free regressions cover
placement, resize and lifecycle; native headset screenshots remain required.
