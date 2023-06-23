#!/bin/bash

while true
do
    RED=$((RANDOM % 256))
    GREEN=$((RANDOM % 256))
    BLUE=$((RANDOM % 256))

    echo "running $RED:$GREEN:$BLUE"

    cargo run --release -q -- -m jun -y 2023 data -r $RED -g $GREEN -b $BLUE -o ~/Desktop/chart.png

    qlmanage -p ~/Desktop/chart.png &>/dev/null
done
