"""A running tangoAW2 console: aw2_script in interactive mode (`-` script).

Every command is one line; the runner answers with its output lines and then
`@ok FRAME`. Frames only advance on wait/press/hold/goto/until commands, so
the game is fully deterministic for a given ROM, save and command list.
"""

import os
import subprocess
import tempfile

from . import paths

KEYS = ("A", "B", "SELECT", "START", "RIGHT", "LEFT", "UP", "DOWN", "R", "L")


class EmuError(RuntimeError):
    pass


class Emu:
    def __init__(self, save=None, rom=None, ds=False, log=None, trace=None):
        self.rom = rom or paths.aw2_rom()
        env = dict(os.environ)
        if ds:
            env["TANGOAW2_DS_ROM"] = paths.ds_rom()
        else:
            env.pop("TANGOAW2_DS_ROM", None)
        if trace:
            env["AW2_TRACE"] = trace
        cmd = [paths.runner("aw2_script"), self.rom, "-"]
        if save:
            cmd += ["--save", save]
        self.proc = subprocess.Popen(
            cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=env, bufsize=1
        )
        self.frame = 0
        self.log = log
        self.traps = []
        # Every frame-advancing command, in order (wait/press/hold, and pokes),
        # so a run can be replayed by another runner (aw2_netplay_script).
        self.timeline = []
        _live.add(self)

    # -- protocol ---------------------------------------------------------
    def cmd(self, line):
        """Send one command; return its output lines (without the @ok line)."""
        if self.log:
            self.log.write(line + "\n")
        self.proc.stdin.write(line + "\n")
        self.proc.stdin.flush()
        out = []
        while True:
            got = self.proc.stdout.readline()
            if not got:
                raise EmuError(f"runner exited during {line!r} (output so far: {out})")
            got = got.rstrip("\n")
            if got.startswith("@ok "):
                self.frame = int(got[4:])
                break
            if got.startswith("trap "):
                self.traps.append(got)
                continue
            out.append(got)
        return out

    def close(self):
        if self.proc.poll() is None:
            try:
                self.proc.stdin.write("quit\n")
                self.proc.stdin.close()
                self.proc.stdout.read()
                self.proc.wait(timeout=10)
            except Exception:
                self.proc.kill()

    def __enter__(self):
        return self

    def __exit__(self, *a):
        self.close()

    # -- input ------------------------------------------------------------
    def wait(self, n):
        if n > 0:
            self.cmd(f"wait {n}")
            if self.timeline and self.timeline[-1].startswith("wait "):
                self.timeline[-1] = f"wait {int(self.timeline[-1][5:]) + n}"
            else:
                self.timeline.append(f"wait {n}")

    def press(self, keys, n=2):
        """Hold KEYS for n frames, then release for 6."""
        self.cmd(f"press {keys} {n}")
        self.timeline.append(f"press {keys} {n}")

    def hold(self, keys, n):
        self.cmd(f"hold {keys} {n}")
        self.timeline.append(f"hold {keys} {n}")

    # -- memory -----------------------------------------------------------
    def read(self, addr, n):
        out = self.cmd(f"peek {addr:08x} {n}")
        line = [l for l in out if ":" in l][-1]
        hexs = line.split(":", 1)[1].split()
        return bytes(int(h, 16) for h in hexs)

    def u8(self, addr):
        return self.read(addr, 1)[0]

    def u16(self, addr):
        return int.from_bytes(self.read(addr, 2), "little")

    def s16(self, addr):
        v = self.u16(addr)
        return v - 0x10000 if v & 0x8000 else v

    def u32(self, addr):
        return int.from_bytes(self.read(addr, 4), "little")

    def w8(self, addr, v):
        self.cmd(f"poke8 {addr:08x} {v & 0xFF:x}")
        self.timeline.append(f"poke8 {addr:08x} {v & 0xFF:x}")

    def w16(self, addr, v):
        self.cmd(f"poke16 {addr:08x} {v & 0xFFFF:x}")
        self.timeline.append(f"poke16 {addr:08x} {v & 0xFFFF:x}")

    def w32(self, addr, v):
        self.cmd(f"poke32 {addr:08x} {v & 0xFFFFFFFF:x}")
        self.timeline.append(f"poke32 {addr:08x} {v & 0xFFFFFFFF:x}")

    def write(self, addr, data):
        self.cmd(f"pokebytes {addr:08x} {bytes(data).hex()}")
        self.timeline.append(f"pokebytes {addr:08x} {bytes(data).hex()}")

    def until8(self, addr, val, max_frames, mask=0xFF, ne=False):
        """Run frames until (byte & mask) == val (or != val); True if it happened."""
        op = "untilne8" if ne else "until8"
        before = self.frame
        out = self.cmd(f"{op} {addr:08x} {val:x} {max_frames} {mask:x}")
        if self.frame > before:
            self.timeline.append(f"wait {self.frame - before}")
        return any(" hit " in l for l in out)

    def wait_until(self, pred, max_frames, step=4):
        """Run frames in `step`-frame slices until pred() is true."""
        n = 0
        while n <= max_frames:
            if pred():
                return True
            self.wait(step)
            n += step
        return pred()

    def shot(self, path):
        """Write a 240x160 BMP (path without .bmp)."""
        self.cmd(f"shot {path}")
        return path + ".bmp"

    def audio_start(self):
        """Start recording the console's sound output."""
        self.cmd("audio")

    def audio_end(self, path):
        """Stop recording; write a 16-bit stereo WAV to path. Returns (frames, rate)."""
        out = self.cmd(f"audioend {path}")
        parts = [l for l in out if l.startswith("audio ")][-1].split()
        return int(parts[1]), int(parts[4])

    def dump_rom(self, path, length=0x800000):
        self.cmd(f"dumprange 08000000 {length} {path}")
        with open(path, "rb") as f:
            return f.read()

    def save(self, path):
        """Export the cartridge save to path (without .sav)."""
        self.cmd(f"save {path}")
        return path + ".sav"


import atexit as _atexit
import weakref as _weakref

_live = _weakref.WeakSet()


@_atexit.register
def _close_all():
    for e in list(_live):
        e.close()
