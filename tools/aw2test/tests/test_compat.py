"""Byte-for-byte comparison with another build of tangoAW2.

Set AW2TEST_COMPARE_RUNNER to an aw2_script binary built from an older commit
(e.g. the one before a feature) to check that, without the Dual Strike pack,
the current build plays exactly as that one did: the same battle, powers,
random weather and CPU turns are replayed on both from the same inputs and all
of EWRAM and IWRAM must match at the end.

    git archive <commit> | tar -x -C /tmp/old && cd /tmp/old &&
    CARGO_TARGET_DIR=/tmp/old/target cargo build --release -p tango-gamesupport-aw2 --example aw2_script
    AW2TEST_COMPARE_RUNNER=/tmp/old/target/release/examples/aw2_script python3 tools/aw2test/run.py -k compat
"""

import os

from aw2test import paths
from aw2test.harness import Skip, test

SNOW_CHANCE = 0x03004492


@test(modes=("aw2",))
def compat_aw2_byte_identical(ctx):
    other = os.environ.get("AW2TEST_COMPARE_RUNNER")
    if not other:
        raise Skip("AW2TEST_COMPARE_RUNNER not set")
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 11, 10).unit(1, "artillery", 8, 10).unit(2, "bcopter", 14, 12)
    m.terrain(11, 10, "wood")
    g = ctx.start(m, ["olaf", "drake"], weather="random")
    ctx.attack(g, (10, 10), (10, 10), (11, 10))
    ctx.power(g, 1, "power")                   # Blizzard: snow
    ctx.attack(g, (8, 10), (8, 10), (11, 10))
    g.e.w8(SNOW_CHANCE, 100)
    g.end_turn(human=1)
    g.end_turn(human=1)
    g.e.wait(30)
    live = g.e.read(0x02000000, 0x40000)
    script = ctx.script(g, "replay.txt", tail=["dump end", "shot end"])
    save = os.path.join(ctx.out, "map.sav")
    dumps = {}
    for tag, runner in (("this build", paths.runner("aw2_script")), ("other build", other)):
        out = ctx.run_script(runner, script, save)
        ctx.log(f"{tag}: {out.strip().splitlines()[-1] if out.strip() else ''}")
        dumps[tag] = [open(os.path.join(ctx.out, "end" + ext), "rb").read() for ext in (".ewram", ".iwram")]
        for ext in (".ewram", ".iwram", ".bmp"):
            os.replace(os.path.join(ctx.out, "end" + ext), os.path.join(ctx.out, f"end_{tag.split()[0]}{ext}"))
    units = slice(g.units_base - 0x02000000, g.units_base - 0x02000000 + 12 * 256)
    ctx.check(dumps["this build"][0][units] == live[units], "the file-script replay ends with the live run's units")
    for i, (name, base) in enumerate((("EWRAM", 0x02000000), ("IWRAM", 0x03000000))):
        a, b = dumps["this build"][i], dumps["other build"][i]
        diff = [base + k for k in range(len(a)) if a[k] != b[k]]
        ctx.check(not diff, f"{name} identical to the other build ({len(diff)} bytes differ: "
                            f"{', '.join(hex(x) for x in diff[:12])})")
