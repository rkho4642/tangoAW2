"""Where the ROMs, runners and scratch output live (all overridable by env)."""

import os

HERE = os.path.dirname(os.path.abspath(__file__))
TOOL = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(TOOL))


def aw2_rom():
    return os.environ.get(
        "AW2TEST_ROM", os.path.expanduser("~/Documents/TangoAW2/roms/Advance_wars_2.gba")
    )


def ds_rom():
    return os.environ.get(
        "TANGOAW2_DS_ROM",
        os.path.expanduser("~/Documents/TangoAW2/roms/Advance Wars - Dual Strike (USA).nds"),
    )


def runner(name):
    return os.path.join(REPO, "target", "release", "examples", name)


def out_dir(*parts):
    """Scratch output (gitignored): tools/aw2test/out/..., or $AW2TEST_OUT."""
    base = os.environ.get("AW2TEST_OUT", os.path.join(TOOL, "out"))
    d = os.path.join(base, *parts)
    os.makedirs(d, exist_ok=True)
    return d


LIVE_SAVE = os.path.expanduser("~/Documents/TangoAW2/saves/Advance Wars 2.sav")


def base_save():
    """A cartridge save to start from: any save past the campaign prologue (it is
    only read; the harness writes its maps into a copy). $AW2TEST_BASE_SAVE, or
    a pinned copy in the output folder (`base.sav`), taken once from the
    player's own save (which is never written, and which changes as they
    play: a pinned copy keeps every run starting from the same state)."""
    p = os.environ.get("AW2TEST_BASE_SAVE")
    if p:
        return p
    pinned = os.path.join(out_dir(), "base.sav")
    if not os.path.exists(pinned):
        import shutil
        shutil.copyfile(LIVE_SAVE, pinned)
    return pinned
