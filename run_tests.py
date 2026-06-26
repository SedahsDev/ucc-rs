#!/usr/bin/env python3
"""Run ucc-rs tests with UCC library path configured."""
import subprocess
import os
import sys

ucc_lib_dir = "/home/bzf/projects/ucc/src/.libs"

# Build the env with library path
env = os.environ.copy()
existing = env.get("LD_LIBRARY_PATH", "")
if existing:
    env["LD_LIBRARY_PATH"] = ucc_lib_dir + ":" + existing
else:
    env["LD_LIBRARY_PATH"] = ucc_lib_dir

# Run cargo test
result = subprocess.run(
    ["cargo", "test", "--lib"] + sys.argv[1:],
    env=env,
    cwd="/home/bzf/projects/ucc-rs",
)
sys.exit(result.returncode)
