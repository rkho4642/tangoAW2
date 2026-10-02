"""CO tag pairs (tangoAW2's crate::tag): RAM layout and helpers for tests.

Dual Strike's numbers (compatibility) are read from the .nds directly
(rom.DualStrike), never from the running game.
"""

import struct

from . import rom as romlib

STATE = 0x0203F400
REC = 0x20
P_CO, P_PHASE, P_USES, P_ANNOUNCE, P_CHARGE, P_SKILLS = 0x00, 0x01, 0x02, 0x03, 0x04, 0x08
RULE = STATE + 0xA0
TEAMS_PARTNER = STATE + 0xA4
PANEL = STATE + 0xAC
MAGIC_AT = STATE + 0xFC
NONE = 0xFF

# The map menu's copy (tangoAW2's) and its pool word.
MENU_POOL = 0x0802D49C
TAG_MENU = 0x08780000


def rec(army):
    return STATE + REC * (army - 1)


def partner(e, army):
    """(CO, phase, uses, charge) of army's partner, or None."""
    b = e.read(rec(army), REC)
    if b[P_CO] == NONE or e.u8(MAGIC_AT) != 0x7A:
        return None
    return {"co": b[P_CO], "phase": b[P_PHASE], "uses": b[P_USES], "announce": b[P_ANNOUNCE],
            "charge": struct.unpack_from("<I", b, P_CHARGE)[0], "skills": bytes(b[P_SKILLS:P_SKILLS + 6])}


def set_rule(e, on):
    e.w8(RULE, 1 if on else 0)


def set_teams_partner(e, army, co):
    e.w8(TEAMS_PARTNER + army - 1, NONE if co is None else romlib.co_id(co))


def star_cost(uses):
    pct = 200 if uses > 9 else 100 + 20 * uses
    return 9000 * pct // 100


def compatibility(ds, a, b):
    """Dual Strike's compatibility of the pair (CO record +0x84 + partner's id)."""
    ra = romlib.DS_CO_IDS.get(a)
    rb = romlib.DS_CO_IDS.get(b)
    if ra is None or rb is None:
        return 100
    return ds.a9(0x0215360C + 0x220 * ra + 0x84 + rb, 1)[0]
