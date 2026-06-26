#!/bin/bash
# Wrapper to run ucc-rs tests with correct library path
export UCC_LIB_DIR="/home/bzf/projects/ucc/src/.libs"
exec cargo test --lib "$@"
