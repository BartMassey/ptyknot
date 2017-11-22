#!/bin/sh
# Copyright © 2017 Bart Massey
if [ ! -d docs ]
then
    echo "no docs directory" >&2
    exit 1
fi
if [ ! -d target/doc ]
then
    echo "no docs" >&2
    exit 1
fi
git rm -rf docs &&
rm -rf docs &&
cp -a target/doc docs &&
git add docs &&
git commit -m 'updated docs'
