"""RAM addresses the harness relies on (AW2 USA with tangoAW2's patches).

Sources: docs/AW2.md, the aw2bhr decompilation, and probing with this harness
(see README.md). Everything is read through these names so a relocation needs
one edit here.
"""

# --- global play state (gPlaySt, 0x03003FC0) --------------------------------
PLAYST = 0x03003FC0
MODE = 0x03003FC1              # sub-mode
VS_MAP = 0x03003FC2            # Versus map id; design maps 0xB4..0xB7
CO_POWERS_ENABLED = 0x03003FC7
CO_ABILITIES = 0x03003FC8
ANIM_OPTS = 0x03003FC9         # battle scenes: 0 off, 1 Type A, 2 Type B, 3 Type C
FOG = 0x03003FCD
FUNDS_PER_PROPERTY = 0x03003FE8
WEATHER = 0x03003FEC           # 0 clear, 1 snow, 2 rain
TITLE_MODE = 0x030033FC        # 1 Campaign, 3 Versus, 5 War Room
MAIN_CALLBACK = 0x03000000     # 0x08022049 on the battle map

# --- battle -------------------------------------------------------------------
CURRENT_ARMY = 0x030033EC      # 1..4
CURSOR_X = 0x030033E4          # map cursor (tile)
CURSOR_Y = 0x030033E6
SELECTED_UNIT = 0x030040D8     # Unit* of the selected unit
MAP_MENU_STATE = 0x030014E2    # 5 = an item was chosen
MAP_MENU_CURSOR = 0x030014F0   # 4 = End (without power items)
BATTLE_ATTACKER = 0x030013D0   # BattleUnit records (+0x10 baseDamage, +0x0C damage...)
BATTLE_DEFENDER = 0x030013B0
BATTLE_SCENE_ROWS = 0x03004580  # 16 bytes per side, set when an attack is confirmed

# Pointer words in the ROM image (read at run time: five-army mode moves players).
UNITS_PTR = 0x08499594         # -> gUnits (256 x 12 bytes; id = (army-1)*64 + slot)
PLAYERS_PTR = 0x08499598       # -> gPlayers (0x3C each, slot 0 unused)
MAP_PTR = 0x0201E450           # gMap (design/battle map): terrain classes at +0x1432
MAP_TERRAIN = 0x0201E450 + 0x1432
MAP_UNITS = 0x0201E450 + 0x12
MAP_ROW_OFFSETS = 0x0201E450 + 0x417A

UNIT_SIZE = 12
PLAYER_SIZE = 0x3C
# Player block fields.
P_COLOUR = 0x1A
P_CO = 0x1D
P_CO_MODE = 0x1E               # 0 d2d, 1 COP, 2 SCOP
P_CO_ACTIVATION = 0x1F
P_CHARGE = 0x20                # u32
P_POWER_NOTICE = 0x24
P_POWERS_USED = 0x25
P_TEMP_FIREPOWER = 0x26        # s16
P_TEMP_DEFENCE = 0x28          # s16
P_FUNDS = 0x00                 # u32 (1000 at the start with Funds 1000)

# --- Versus menus ---------------------------------------------------------------
TEAMS = 0x02017C50             # Teams record (five-army mode: 0x02030300)
T_ARMY_COUNT = 0x08
T_CONTROLLERS = 0x09           # per army: 1 human, 2 computer
T_COLOURS = 0x0D
T_CO_COUNT = 0x17
T_CO_LIST = 0x18               # u32 pointer to the CO list (0xFF-terminated)
T_CO_INDEX = 0x1C              # per army: index into the CO list
RULES_CURSOR = 0x02017C83      # 0 fog .. 6 visuals
PROCS = (0x03001500, 0x03001F00)
TEAMS_PROC_FN = 0x08064E5D
RULES_ITEM_FNS = (0x08064739, 0x08064775, 0x080647BD, 0x0806486D, 0x08064919, 0x080649D1)
RULES_ITEM_VALUE = 0x48        # offset of the value byte in a rules item proc (0x60 bytes)
RULES_ITEM_COUNT = 0x4B
RULES_ITEM_FN = 0x4C
