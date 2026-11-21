"""Renode capture sequence for SQL Explorer.

Run from xous-dev-toolkit:
    python3 scripts/renode_capture.py --app sql-explorer --script ../precursor-sqlite/capture.py

Contract: run(ctl, outdir) -> number of failed steps. The app is already launched.
Not yet run: written against the toolkit's RenodeController API.
"""
import os


def run(ctl, outdir) -> int:
    failures = 0

    def shot(name):
        nonlocal failures
        if not ctl.try_screenshot(os.path.join(outdir, name)):
            failures += 1

    shot("01_query.png")
    ctl.timed_key("F3", after=1.0)       # what is supported
    shot("02_help.png")
    ctl.timed_key("BackSpace", after=1.0)
    ctl.inject_string("SELECT name, age FROM users WHERE age > 28 ORDER BY age")
    shot("03_typed.png")
    ctl.inject_line("")                  # Enter: run
    ctl.sleep(1.0)
    shot("04_results.png")
    ctl.timed_key("BackSpace", after=1.0)
    ctl.timed_key("F1", after=1.0)       # sample queries
    shot("05_samples.png")
    return failures
