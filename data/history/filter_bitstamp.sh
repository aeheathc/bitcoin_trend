#!/bin/bash

# This script converts the huge csv files from kaggle into small ones suitable for the main program.

if [ $# -ne 1 ]
  then
    echo "Usage: filter_bitstamp.sh inputfile"
	exit
fi

if [ ! -f $1 ]
  then
    echo "Couldn't find file $1"
	exit
fi

# Keep only every 60th line, this gives us hourly rather than minutely data
# Eliminate columns other than timestamp and average_price
# Exclude NaN rows
cat $1 | awk 'NR == 1 || NR % 60 == 0' | sed -E -e 's/,.*,.*,.*,.*,.*,.*,/,/' | sed -E -e '/NaN|Time/d' > bitstamp.csv