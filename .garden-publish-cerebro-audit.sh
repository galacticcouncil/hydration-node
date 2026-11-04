#!/bin/sh
set -eu

git -C /home/cl0w/projects/hydration/garden pull --rebase origin main
git -C /home/cl0w/projects/hydration/garden push origin main
