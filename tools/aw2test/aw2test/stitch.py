"""The whole battle map as one picture (tests/test_ds_maps.py, Survival's and
the looks' pictures). Saved in the test's output and, with
AW2TEST_MAP_IMAGES set, in that folder too."""

import os

GMAP = 0x0201E450
# The game's OAM buffer (copied to OAM at VBlank) and the priority its
# panels, cursor and dialogue faces use (0..2; the map's buildings and
# structures are 3).
OAM_BUFFER = 0x03002520


def hide_hud(e):
    """Hides every sprite in front of the map (the CO and terrain panels,
    the cursor) for the next frame drawn: the picture is the map's alone."""
    import struct
    b = bytearray(e.read(OAM_BUFFER, 0x400))
    for k in range(128):
        if (struct.unpack_from("<H", b, 8 * k + 4)[0] >> 10) & 3 < 3:
            struct.pack_into("<H", b, 8 * k, 0x0200 | 160)
    e.write(OAM_BUFFER, bytes(b))
    e.wait(1)
IMAGES = os.environ.get("AW2TEST_MAP_IMAGES")


def stitch(ctx, g, name, w, h, each=None, exclude=None):
    """The whole map as one picture: the cursor sweeps it and a screenshot is
    taken every two cells, the panels and cursor hidden ([`hide_hud`]). Each
    cell is then the medoid of its views (the view nearest all the others).
    `each()` is called at every view; `exclude(tx, ty)` names screen cells
    never taken (a banner fixed on the screen). Needs PIL and numpy (else
    skipped)."""
    try:
        import numpy as np
        from PIL import Image
    except ImportError:
        ctx.log("no PIL/numpy: full-map picture skipped")
        return None
    e = g.e
    views = [[[] for _ in range(w)] for _ in range(h)]
    shots = 0
    ys = sorted(set(list(range(0, h, 2)) + [h - 1]))
    xs = sorted(set(list(range(0, w, 2)) + [w - 1]))
    for i, cy in enumerate(ys):
        for cx in (xs if i % 2 == 0 else xs[::-1]):
            g.goto(cx, cy)
            e.wait(8)
            for _ in range(20):
                camx, camy = e.s16(GMAP + 4), e.s16(GMAP + 6)
                if camx % 16 == 0 and camy % 16 == 0:
                    break
                e.wait(2)
            e.wait(2)
            if each:
                each()
            hide_hud(e)
            path = e.shot(os.path.join(ctx.out, "sweep"))
            shots += 1
            img = np.asarray(Image.open(path).convert("RGB")).astype(np.int32)
            for ty in range(10):
                for tx in range(15):
                    mx, my = camx // 16 + tx, camy // 16 + ty
                    if exclude and exclude(tx, ty):
                        continue
                    if 0 <= mx < w and 0 <= my < h and max(abs(mx - cx), abs(my - cy)) > 1:
                        same = (tx < 7.5) == ((cx * 16 - camx) < 120)
                        views[my][mx].append((same, img[16 * ty:16 * ty + 16, 16 * tx:16 * tx + 16]))
    full = np.zeros((16 * h, 16 * w, 3), dtype=np.uint8)
    missing = 0
    for my in range(h):
        for mx in range(w):
            v = [c for s, c in views[my][mx] if s] or [c for _, c in views[my][mx]]
            if not v:
                missing += 1
                continue
            best = min(range(len(v)), key=lambda k: sum(int(np.abs(v[k] - o).sum()) for o in v))
            full[16 * my:16 * my + 16, 16 * mx:16 * mx + 16] = v[best]
    full = Image.fromarray(full)
    ctx.log(f"{name}: {shots} screenshots stitched")
    ctx.check(missing <= 2, f"{name}: the sweep saw the whole map ({missing} cells unseen)")
    fn = name.lower().replace(" ", "_")
    out = os.path.join(ctx.out, f"{fn}_full.png")
    full.save(out)
    if IMAGES:
        os.makedirs(IMAGES, exist_ok=True)
        full.save(os.path.join(IMAGES, f"{fn}_full.png"))
    return out
