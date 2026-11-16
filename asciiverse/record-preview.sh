#!/bin/sh
# Records the 100x30 catalog preview, asciiverse/wizards-castle.cast
# (hub/RECORDING.md): a Dwarf made and sent into the castle, then played by
# asciiverse/drive.py for about 45 seconds (flares and the lamp light up
# the map, chests, books, pools and orbs are tried, monsters fought),
# then quit. The castle is new every game, so a take that ended early or
# did not show a fight, treasure or gold and a curious room is thrown
# away and played again, up to eight times. asciinema records a tmux pane (own socket, no
# status line, no user config) that drive.py reads and plays into. Needs
# asciinema 3, tmux and Python 3.
#   asciiverse/record-preview.sh target/release/wizards_castle_ncurses
set -eu
binary=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
here=$(cd "$(dirname "$0")" && pwd)
out=$here/wizards-castle.cast
home=$(mktemp -d)
trap 'tmux -L avrec kill-server 2>/dev/null; rm -rf "$home"' EXIT
mkdir "$home/bin" "$home/state" && cp "$binary" "$home/bin/wizards-castle"
cd "$home"  # nothing in the recording names a local path
export HOME="$home" XDG_STATE_HOME="$home/state" PATH="$home/bin:$PATH" \
  USER=player LOGNAME=player TERM=xterm-256color COLORTERM=truecolor LANG=C.UTF-8
unset XDG_CONFIG_HOME RUST_LOG
for take in 1 2 3 4 5 6 7 8; do
  tmux -L avrec kill-server 2>/dev/null || true
  tmux -L avrec -f /dev/null new-session -d -s rec -x 100 -y 30 "wizards-castle" \; set -g status off
  python3 -I "$here/drive.py" avrec rec 45 &
  driver=$!
  asciinema rec --headless --overwrite -q \
    --window-size 100x30 --idle-time-limit 2 \
    --capture-env "TERM,COLORTERM,LANG" --title "Wizard's Castle" \
    --command "tmux -L avrec attach -t rec" "$home/take.cast"
  if wait $driver; then
    mv "$home/take.cast" "$out"
    echo "take $take kept"
    exit 0
  fi
  echo "take $take: not everything shown, again"
done
echo "no good take" >&2
exit 1
