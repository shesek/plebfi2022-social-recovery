#!/bin/bash
#rm -rf dist pkg gh-pages/next/*
#npm run build

mkdir -p gh-pages/next
cp -r dist/* gh-pages/next/
#(cd gh-pages && git add . && git commit -S -m 'Update website' && git push gh gh-pages)
