#!/bin/bash
# usage: measure.sh <name> <env...> -- <command...>
S=${STATE:?set STATE to the compositor state dir}
name=$1; shift
. $S/env.sh
envs=(); while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
env "${envs[@]}" "$@" > $S/runs/$name.json 2> $S/runs/$name.err &
P=$!
sleep 6
timeout 15 spectacle -b -n -f -o $S/runs/$name.png >/dev/null 2>&1
# CPU% over 2 s for the process tree (sum), via pidstat-like /proc sampling
desc() { echo $1; for c in $(ps -eo pid=,ppid= | awk -v p=$1 '$2==p {print $1}'); do desc $c; done; }
pids=$(desc $P)
t1=$(for p in $pids; do awk '{print $14+$15}' /proc/$p/stat 2>/dev/null; done | paste -sd+ | bc)
sleep 2
t2=$(for p in $pids; do awk '{print $14+$15}' /proc/$p/stat 2>/dev/null; done | paste -sd+ | bc)
echo "cpu_percent_2s: $(( (t2 - t1) / 2 ))" >> $S/runs/$name.err
wait $P
echo "== $name"; cat $S/runs/$name.json; grep cpu_percent $S/runs/$name.err
