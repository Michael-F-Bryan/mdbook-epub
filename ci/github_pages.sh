#!/bin/bash
set -ex

# Only upload the built book to github pages if it's a commit to master
if [ "$TRAVIS_BRANCH" = master -a "$TRAVIS_PULL_REQUEST" = false ]; then
  pip3 install --user travis-cargo 
  cargo doc
  travis-cargo doc-upload
fi