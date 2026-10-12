Golden save files for src/sim/save/tests.rs (module `golden`).

They were written by the save code as it was at commit 5348c14 (before the
file split and the refactors), by a throwaway program that built each save from
the public fields and called SaveGame::to_json(). Never regenerate them from
the current code: they exist to prove today's code still reads and writes
exactly what older builds did. Only change them if the save format is
deliberately versioned (and then keep the old ones as well).

default_save.json  SaveGame::default()
rich_save.json     all weapons owned with upgrades, every quest and perk flag,
                   partly looted containers, fog with 3 places found, a
                   blizzard, a non-default clock, a full pack
indoors_save.json  the same game saved inside fish house 1
