"""The looks' reference (aw2test/looks.py) against Dual Strike's own screen.

Dual Strike draws its map in 3D, tilted; captures of it (melonDS, the
bottom screen's 3D layer alone, the cursor swept over every cell, each
camera's view flattened back onto the 16-pixel grid) are kept outside the
repo, as they are pictures of the game: $AW2TEST_DS_FRAMES names a folder
with `manifest.json` (a list of {"name", "mission": DS Campaign mission
index, "look": looks.SNOW/WASTELAND/..., "file": the flattened PNG}). Without
it the test is skipped.

Each cell with no building or unit standing on it or below it (Dual
Strike's buildings and units are on the same layer, and reach into the cell
above) is compared with the reference's drawing of it, clear or fogged
(terrain sub-palettes 0-4 drawn with 5), whichever is nearer: each pixel of
the capture (5-bit colours: the 3D output is 6-bit) against the nearest
colour of the reference's cell (the top rows are seen smaller than 16
pixels, so not pixel for pixel). Dual Strike's drifting cloud shadows and weather
particles are mostly gone from the captures (each pixel the brighter of
several frames) but not everywhere: most cells, not all, must agree."""

import json
import os

from aw2test import dscampaign as dc
from aw2test import looks
from aw2test.harness import test

FRAMES = os.environ.get("AW2TEST_DS_FRAMES")
# Classes drawn as buildings (and structures) in Dual Strike's 3D layer.
BUILDINGS = range(0x0D, 0x14)
# A cell agrees when its pixels' mean squared distance (5-bit channels) to the
# nearest colour of the reference's cell is within this.
GOOD = 6.0


def _cells(ref, m, img):
    w, h = m["w"], m["h"]
    ids = [ref.ds_id(t, i % w, i // w) for i, t in enumerate(m["tiles"])]
    skip = set()
    for i, t in enumerate(m["tiles"]):
        x, y = i % w, i // w
        if ids[i] is None or ref.classes[t] in BUILDINGS or t >= 0x150:
            skip |= {(x, y), (x, y - 1)}
    for u in m["units"]:
        skip |= {(u[1], u[2]), (u[1], u[2] - 1)}
    px = img.load()
    out = {}
    for y in range(h):
        for x in range(w):
            if (x, y) in skip:
                continue
            below = ids[(y + 1) * w + x] if y + 1 < h else None
            below = below if below in looks.TALL else None
            ds = []
            for yy in range(16):
                for xx in range(16):
                    r, g, b = px[16 * x + xx, 16 * y + yy][:3]
                    ds.append(tuple(((v * 63 + 127) // 255) >> 1 for v in (r, g, b)))
            best = None
            for fog in (False, True):
                cell = ref.render_cell(ids[y * w + x], below, fog=fog)
                have = {((c & 31), (c >> 5) & 31, (c >> 10) & 31) for row in cell for c in row if c is not None}
                err = sum(min(sum((p - q) ** 2 for p, q in zip(c, h)) for h in have) for c in ds) / len(ds)
                best = err if best is None else min(best, err)
            out[(x, y)] = best
    return out


@test(modes=("ds",))
def ds_frames_looks(ctx):
    if not FRAMES:
        ctx.log("AW2TEST_DS_FRAMES not set: no Dual Strike captures to compare")
        return
    try:
        from PIL import Image
    except ImportError:
        ctx.log("no PIL: skipped")
        return
    data = dc.DsData()
    for cap in json.load(open(os.path.join(FRAMES, "manifest.json"))):
        m = data.mission(cap["mission"])
        ref = looks.reference(cap["look"])
        img = Image.open(os.path.join(FRAMES, cap["file"])).convert("RGB")
        cells = _cells(ref, m, img)
        errs = sorted(cells.values())
        good = sum(e <= GOOD for e in errs) / len(errs)
        worst = sorted(cells.items(), key=lambda c: -c[1])[:5]
        ctx.log(f"{cap['name']}: {len(errs)} cells, median error {errs[len(errs) // 2]:.2f}, "
                f"{good:.0%} of cells within {GOOD}; worst {[(c, round(e, 1)) for c, e in worst]}")
        ctx.check(errs[len(errs) // 2] <= 3 and good >= 0.6,
                  f"{cap['name']}: Dual Strike's screen drawn as the reference draws it")
