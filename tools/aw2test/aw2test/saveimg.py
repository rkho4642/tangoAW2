"""The cartridge save as AW2 reads it: sectors, slots (tags), the newest copy
of each, and diffs between two images (for the save-integrity tests,
tests/test_save_integrity.py).

AW2's Flash (64 KiB) is 16 sectors of 0x1000 bytes. Each written sector is
one part of a slot (its tag): "2ars" at +0, 0x55/0xAA at +4 with the other
at +0xFFF, 0x0F at +5, the 8-bit sum of the whole sector at +6 and its
complement at +7 (sub_0801B09C, AW2's validator), the generation at +8,
part index << 4 | (parts - 1) at +0xC, the tag at +0xD, the payload's place
at +0xE, its length at +0x50, the payload from +0x52; the newest profile
(tag 0) lists every sector's tag at +0xFEF (the directory, sub_0801B2FC).
The writer (sub_0801A7D8) puts a slot's new copy in free sectors and then a
new profile, which is serialized from RAM every time (sub_08016B2C).

Tags: 0 profile (0x5CC bytes), 2 / 3 / 4 the Campaign / War Room / Versus
suspend (0xE28), 5..7 design maps 1..3 (0x724), 8 the design map a
suspended Versus game is played on (0x724), 15 tangoAW2's DS Campaign
record (crate::ds_campaign::SAVE_SLOT).
"""

import struct

SECTOR = 0x1000
MAGIC = 0x73726132

TAG_NAMES = {0: "profile", 2: "campaign suspend", 3: "war room suspend", 4: "versus suspend",
             5: "design 1", 6: "design 2", 7: "design 3", 8: "suspended design map", 15: "DS Campaign"}

# The profile (sub_08016B2C): where each part comes from.
PROFILE_PARTS = (
    ("unlocks", 0x000, 0x048),       # 0x02028030: campaign flags, maps, COs, shop, played bits
    ("war room scores", 0x048, 0x2A0),   # 0x0200C078: 30 maps x 5 records
    ("campaign scores", 0x2A0, 0x3F0),   # 0x0200C2D0: 42 missions x (normal, hard)
    ("options", 0x3F0, 0x4D0),       # 0x0200C420: points, save count, suspend flags, options, results
    ("world map", 0x4D0, 0x5CC),     # 0x0202FDFC: AW2's campaign map state
)
P_C420 = 0x3F0
# Bytes of 0x0200C420 (profile + 0x3F0 + offset).
C420_SAVE_COUNT = 0x08         # bumped by sub_08016A14 before each save while even
C420_SUSPEND = {2: 0x09, 3: 0x0A, 4: 0x0B}   # a suspended game per mode (sub_08016C9C)
C420_SURVIVAL = range(0x15, 0x1F)            # tangoAW2's Survival records (0x0200C435..)


def valid(sec):
    """AW2's own sector check (sub_0801B09C): True when it accepts it."""
    if struct.unpack_from("<I", sec, 0)[0] != MAGIC:
        return False
    if sec[4] == 0x55:
        if sec[0xFFF] != 0xAA:
            return False
    elif sec[4] == 0xAA:
        if sec[0xFFF] != 0x55:
            return False
    else:
        return False
    s = sum(sec) & 0xFF
    return sec[6] == s and sec[7] == (~s) & 0xFF and sec[5] == 0x0F


class Image:
    def __init__(self, data):
        self.path = data if isinstance(data, str) else None
        if isinstance(data, str):
            with open(data, "rb") as f:
                data = f.read()
        self.data = bytes(data)
        assert len(self.data) == 0x10000, len(self.data)
        self.sectors = []
        for s in range(16):
            sec = self.data[s * SECTOR:(s + 1) * SECTOR]
            if struct.unpack_from("<I", sec, 0)[0] != MAGIC:
                self.sectors.append(None)
                continue
            self.sectors.append({
                "sector": s, "ok": valid(sec), "gen": struct.unpack_from("<I", sec, 8)[0],
                "part": sec[0xC], "tag": sec[0xD], "dst": struct.unpack_from("<H", sec, 0xE)[0],
                "len": struct.unpack_from("<H", sec, 0x50)[0], "dir": sec[0xFEF:0xFFF],
                "payload": sec[0x52:0x52 + struct.unpack_from("<H", sec, 0x50)[0]],
            })

    def newest_profile(self):
        best = None
        for s in self.sectors:
            if s and s["ok"] and s["tag"] == 0 and (best is None or s["gen"] > best["gen"]
                                                     or s["gen"] == best["gen"] and s["part"] < best["part"]):
                best = s
        return best

    def directory(self):
        """Sector -> tag as the newest profile lists it (0xFF free)."""
        p = self.newest_profile()
        return list(p["dir"]) if p else [0xFF] * 16

    def tags(self):
        """The slots in use (the directory's tags, the profile's included)."""
        return sorted({t for t in self.directory() if t != 0xFF} | ({0} if self.newest_profile() else set()))

    def slot(self, tag):
        """The slot's payload as AW2 loads it (sub_0801AC58: the sectors the
        directory lists for `tag`, the newest complete copy), or None."""
        if tag == 0:
            p = self.newest_profile()
            return bytes(p["payload"]) if p else None
        listed = [s for i, s in enumerate(self.sectors) if self.directory()[i] == tag and s and s["tag"] == tag]
        if not listed:
            return None
        gen = max(s["gen"] for s in listed)
        parts = sorted((s for s in listed if s["gen"] == gen), key=lambda s: s["part"] >> 4)
        out = bytearray()
        for s in parts:
            end = s["dst"] + s["len"]
            if len(out) < end:
                out += bytes(end - len(out))
            out[s["dst"]:end] = s["payload"]
        return bytes(out)

    def problems(self):
        """What AW2 would reject: every sector the directory lists must pass
        its check and carry the tag listed; each listed slot must have one
        complete newest copy."""
        out = []
        p = self.newest_profile()
        if p is None:
            return ["no valid profile"]
        d = self.directory()
        for i, t in enumerate(d):
            if t == 0xFF:
                continue
            s = self.sectors[i]
            if s is None or not s["ok"]:
                out.append(f"sector {i} (listed as tag {t}) fails AW2's check")
            elif s["tag"] != t:
                out.append(f"sector {i} listed as tag {t} holds tag {s['tag']}")
        for t in set(d) - {0xFF, 0}:
            listed = [self.sectors[i] for i in range(16) if d[i] == t and self.sectors[i]]
            if listed:
                gen = max(s["gen"] for s in listed)
                parts = [s for s in listed if s["gen"] == gen]
                want = (parts[0]["part"] & 0xF) + 1
                if len(parts) != want:
                    out.append(f"tag {t}: {len(parts)} of {want} parts")
        return out

    def slots(self):
        return {t: self.slot(t) for t in self.tags()}


def ranges(a, b):
    """The byte ranges [(start, end)] where a and b differ (a longer one's tail too)."""
    out = []
    n = max(len(a), len(b))
    k = 0
    while k < n:
        if k < len(a) and k < len(b) and a[k] == b[k]:
            k += 1
            continue
        s = k
        while k < n and not (k < len(a) and k < len(b) and a[k] == b[k]):
            k += 1
        out.append((s, k))
    return out


def profile_part(off):
    for name, lo, hi in PROFILE_PARTS:
        if lo <= off < hi:
            return name
    return "past the profile"


def describe_profile(a, b):
    """The profile bytes that differ, by part: ["options +0x08", ...]."""
    out = []
    for s, e in ranges(a or b"", b or b""):
        name = profile_part(s)
        lo = next(lo for n, lo, hi in PROFILE_PARTS if n == name) if name != "past the profile" else 0
        out.append(f"{name} +{s - lo:#x}" + (f"..{e - 1 - lo:#x}" if e - s > 1 else ""))
    return out


def diff(before, after):
    """Per slot: None (unchanged), or the changed byte ranges of its payload
    ("new" / "gone" when it appears / disappears)."""
    a, b = before.slots(), after.slots()
    out = {}
    for t in sorted(set(a) | set(b)):
        if t not in a:
            out[t] = "new"
        elif t not in b:
            out[t] = "gone"
        elif a[t] != b[t]:
            out[t] = ranges(a[t], b[t])
    return out


def profile_changes(before, after, allow=()):
    """Profile bytes that changed, except the ranges in `allow` (profile
    offsets (start, end)) and the save counter AW2 bumps on every save."""
    pa, pb = before.slot(0), after.slot(0)
    allowed = [(P_C420 + C420_SAVE_COUNT, P_C420 + C420_SAVE_COUNT + 1)] + list(allow)
    bad = []
    for s, e in ranges(pa, pb):
        for k in range(s, e):
            if not any(lo <= k < hi for lo, hi in allowed):
                bad.append(k)
    return bad


def fmt_offsets(offs):
    """Profile offsets as readable runs."""
    if not offs:
        return "none"
    runs = []
    s = p = offs[0]
    for k in offs[1:] + [None]:
        if k is not None and k == p + 1:
            p = k
            continue
        name = profile_part(s)
        lo = next((lo for n, lo, hi in PROFILE_PARTS if n == name), 0)
        runs.append(f"{name} +{s - lo:#x}" + (f"..+{p - lo:#x}" if p > s else ""))
        if k is not None:
            s = p = k
    return ", ".join(runs)
