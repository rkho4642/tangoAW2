#!/usr/bin/env python3
"""Run the tangoAW2 battle tests.

    python3 tools/aw2test/run.py                 # every test, AW2 rules and Dual Strike pack
    python3 tools/aw2test/run.py --mode aw2      # AW2 only (no Dual Strike ROM needed)
    python3 tools/aw2test/run.py -k tank -j 4    # tests whose name contains "tank", 4 at a time
    python3 tools/aw2test/run.py --list

Output (logs, screenshots, saves) goes to tools/aw2test/out/<mode>/<test>/.
"""

import argparse
import concurrent.futures
import glob
import importlib.util
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from aw2test import harness, paths  # noqa: E402


def load_tests():
    for path in sorted(glob.glob(os.path.join(HERE, "tests", "test_*.py"))):
        spec = importlib.util.spec_from_file_location(os.path.basename(path)[:-3], path)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
    return harness.TESTS


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mode", choices=("aw2", "ds", "both"), default="both")
    ap.add_argument("-k", default="", help="only tests whose name contains this")
    ap.add_argument("-x", default="", help="leave out tests whose name contains this (ds_campaign_win_: the long pad-won missions)")
    ap.add_argument("-j", type=int, default=max(1, (os.cpu_count() or 2) // 2))
    ap.add_argument("--list", action="store_true")
    ap.add_argument("-v", action="store_true", help="print each test's log")
    a = ap.parse_args()

    tests = [t for t in load_tests() if a.k in t["name"] and not (a.x and a.x in t["name"])]
    modes = ("aw2", "ds") if a.mode == "both" else (a.mode,)
    if "ds" in modes and not os.path.exists(paths.ds_rom()):
        print(f"no Dual Strike ROM at {paths.ds_rom()} (set TANGOAW2_DS_ROM); running AW2 only")
        modes = ("aw2",)
    jobs = [(t, m) for t in tests for m in modes if m in t["modes"]]
    if a.list:
        for t, m in jobs:
            print(f"{m:4} {t['name']}")
        return 0
    for need in ("aw2_script",):
        if not os.path.exists(paths.runner(need)):
            print(f"missing {paths.runner(need)}: cargo build --release -p tango-gamesupport-aw2 --examples")
            return 2
    t0 = time.time()
    results = []
    with concurrent.futures.ThreadPoolExecutor(a.j) as pool:
        futs = {pool.submit(harness.run_one, t, m): (t, m) for t, m in jobs}
        for f in concurrent.futures.as_completed(futs):
            r = f.result()
            results.append(r)
            status = "SKIP" if r.get("skipped") else "PASS" if r["ok"] else "FAIL"
            print(f"{status} {r['mode']:4} {r['name']:36} {r['checks']:3} checks  {r['out']}", flush=True)
            if not r["ok"]:
                for msg in r["failures"]:
                    print(f"       - {msg}")
                if r["error"]:
                    print("       " + r["error"].strip().splitlines()[-1])
            if r.get("skipped"):
                print(f"       ({r['skipped']})")
            if a.v:
                print(open(os.path.join(r["out"], "log.txt")).read())
    bad = [r for r in results if not r["ok"]]
    print(f"\n{len(results) - len(bad)}/{len(results)} passed in {time.time() - t0:.0f} s")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
