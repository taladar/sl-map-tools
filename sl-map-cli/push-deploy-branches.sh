#!/bin/bash

set -e -u

for b in centos6 centos7 centos8 stretch buster bullseye bookworm
do
  echo "Pushing build_${b}" >&2
  git push origin HEAD:refs/heads/build_${b}
done
