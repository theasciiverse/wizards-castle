#!/usr/bin/env python3
"""drive.py SOCKET SESSION [SECONDS]: plays a game of Wizard's Castle in a
tmux pane, reading the screen the way a player would.

A strong, nimble Dwarf with leather armour, a mace, a lamp and ten
flares. On the map it lights the dark with flares and the lamp, then walks
to the nearest thing worth seeing: a treasure first, then a chest, a book,
a pool or a crystal orb (and opens, reads, drinks or gazes), a monster to
fight (with the mace; it runs from balrogs, dragons and chimeras, and
when weak),
else the nearest unexplored room, and stairs down when a level is done.
After about SECONDS (default 45) it quits.
Character creation goes quickly, so the castle shows within seconds. Run by record-preview.sh while
asciinema records the pane.

Exits 0 when the run lasted and showed a fight, a treasure or gold, and a
curious room, so record-preview.sh can keep the take; 1 otherwise."""
import random, re, subprocess, sys, time

SOCK, S = sys.argv[1], sys.argv[2]
PLAY_SECONDS = float(sys.argv[3]) if len(sys.argv) > 3 else 45
rng = random.Random(1980)

shown = {"fight": False, "treasure": False, "curious": False}


def tmux(*a): return subprocess.run(["tmux", "-L", SOCK, *a], capture_output=True, text=True).stdout
def key(k): tmux("send-keys", "-l", "-t", S, k)
def enter(): tmux("send-keys", "-t", S, "Enter")
def pane(): return tmux("capture-pane", "-p", "-t", S).split("\n")


def pause(a, b=None):
    time.sleep(rng.uniform(a, b if b is not None else a * 1.3))


def has(text, s): return any(s in line for line in text)


def number_after(text, after):
    for line in text:
        i = line.find(after)
        if i >= 0:
            m = re.search(r"\d+", line[i + len(after):])
            if m:
                return int(m.group())
    return None


def read_map(text):
    """The map as an 8x8 grid of characters and the player's position, or
    None when no map is showing. Rows are y, columns x, as the game draws
    them: north is up."""
    for top, line in enumerate(text):
        left = line.find("┌")
        if left >= 0:
            break
    else:
        return None
    grid, pos = [], None
    for r in range(8):
        y = top + 1 + 2 * r
        if y >= len(text):
            return None
        line = text[y]
        row = []
        for c in range(8):
            x = left + 2 + 6 * c
            if x + 1 >= len(line):
                return None
            row.append(line[x])
            if line[x - 1] == "<" and line[x + 1] == ">":
                pos = (c, r)
        grid.append(row)
    if pos is None or not all(ch in "?.EDUGPCFWSOBVMT-" for row in grid for ch in row):
        return None
    return grid, pos


def step_towards(pos, goal):
    """The direction key for one step from pos to goal, round the edges
    when that is shorter (the castle wraps)."""
    (x, y), (gx, gy) = pos, goal
    dx = (gx - x) % 8
    dy = (gy - y) % 8
    moves = []
    if dx:
        moves.append("E" if dx <= 4 else "W")
    if dy:
        moves.append("S" if dy <= 4 else "N")
    return rng.choice(moves) if moves else None


def dist(a, b):
    dx = abs(a[0] - b[0]); dy = abs(a[1] - b[1])
    return min(dx, 8 - dx) + min(dy, 8 - dy)


visited_specials = set()   # (level, x, y) of chests, books, pools, orbs used
level_done = set()
flares_left = 10
lamp_uses = 0
cast_spell = False
start = time.time()


def level_of(text):
    n = number_after(text, ") level")
    return n


def choose_on_map(text):
    global flares_left, lamp_uses
    m = read_map(text)
    if not m:
        if has(text, "** BLIND **"):
            return rng.choice("NSWE")
        return None
    grid, pos = m
    lvl = level_of(text)
    st = number_after(text, "ST:") or 0

    # Something to do in this room
    if has(text, "[O]pen") and (lvl, pos) not in visited_specials:
        visited_specials.add((lvl, pos))
        shown["curious"] = True
        return "O"
    if has(text, "[G]aze") and (lvl, pos) not in visited_specials:
        visited_specials.add((lvl, pos))
        shown["curious"] = True
        return "G"
    if has(text, "[D]rink") and (lvl, pos) not in visited_specials:
        visited_specials.add((lvl, pos))
        shown["curious"] = True
        return "D"

    unknown = sum(row.count("?") for row in grid)

    # Light up the dark round us
    if flares_left > 0 and unknown > 20 and (flares_left == 10 or rng.random() < 0.6):
        x, y = pos
        around = sum(grid[(y + dy) % 8][(x + dx) % 8] == "?" for dx in (-1, 0, 1) for dy in (-1, 0, 1))
        if around >= 4:
            flares_left -= 1
            return "F"

    # Where to go
    targets = []
    for y in range(8):
        for x in range(8):
            ch = grid[y][x]
            here = (x, y)
            if here == pos:
                continue
            if ch == "T":
                targets.append((0, dist(pos, here), here))
            elif ch in "CBPO" and (lvl, here) not in visited_specials:
                targets.append((1, dist(pos, here), here))
            elif ch == "G":
                targets.append((2, dist(pos, here), here))
            elif ch == "M" and st >= 12:
                targets.append((2, dist(pos, here), here))
            elif ch == "?":
                targets.append((3, dist(pos, here), here))
    targets.sort()

    if not targets or (targets[0][0] == 3 and unknown < 30 and lvl not in level_done and rng.random() < 0.3):
        # Little left to see here: go down
        level_done.add(lvl)
        if has(text, "[D]own"):
            return "D"
        stairs = [(dist(pos, (x, y)), (x, y)) for y in range(8) for x in range(8) if grid[y][x] == "D"]
        if stairs:
            return step_towards(pos, min(stairs)[1])

    if not targets:
        return rng.choice("NSEW")

    goal = targets[0][2]

    # Peek ahead with the lamp now and then
    if lamp_uses < 3 and targets[0][0] == 3 and rng.random() < 0.15:
        lamp_uses += 1
        return "L"

    d = step_towards(pos, goal)
    # Don't walk out of the castle by mistake
    if d == "N" and grid[pos[1]][pos[0]] == "E":
        d = rng.choice("EW")
    return d


def choose(text):
    global cast_spell
    if has(text, "Press any key"):
        return " "
    if has(text, "You may be an:"):
        return "D"
    if has(text, "Which sex do you prefer?"):
        return rng.choice("FM")
    if has(text, "How many points do you add to STRENGTH"):
        return "4"
    if has(text, "How many points do you add to INTELLIGENCE"):
        return "0"
    if has(text, "How many points do you add to DEXTERITY"):
        return "4"
    if has(text, "list of armor you can buy"):
        return "L"
    if has(text, "list of weapons you can buy"):
        return "M"
    if has(text, "Want to buy a lamp"):
        return "Y"
    if has(text, "How many do you want?"):
        return "10\n"
    if has(text, "Do you really want to exit"):
        return "N"
    if has(text, "Do you really want to quit"):
        return "Y"
    if has(text, "[P]lay again or [Q]uit"):
        return "Q"
    if has(text, "[C]ontinue"):
        return "C"
    if has(text, "I want The"):
        return "Y"
    if has(text, "What do you want to sell?") or has(text, "What equipment would you like") \
            or has(text, "buy some potions") or has(text, "Teleport to where?"):
        return "N"
    if has(text, "You already have"):
        return "N"
    if has(text, "Shine lamp which way?"):
        return rng.choice("NSWE")
    if has(text, "[F]ireball"):
        return "F"
    if has(text, "to retreat"):
        shown["fight"] = True
        st = number_after(text, "ST:") or 0
        big = any(has(text, "facing " + m) for m in ("A BALROG", "A DRAGON", "A CHIMERA"))
        if has(text, "[C]ast spell") and not cast_spell:
            cast_spell = True
            return "C"
        if st <= 8 or big:
            return rng.choice("NSWE")
        return "A"
    if has(text, "[T]rade"):
        # A vendor: walk on by
        return rng.choice("NSWE")
    return choose_on_map(text)


time.sleep(1.5)
in_game = False
quitting = False
last_log = []
while True:
    text = pane()
    log_text = "\n".join(text)
    if "It's now yours" in log_text or "gold pieces!" in log_text or "You found gold" in log_text \
            or "GREAT ZOT" in log_text:
        shown["treasure"] = True
    if not quitting and time.time() - start > PLAY_SECONDS and read_map(text) and not has(text, "to retreat"):
        quitting = True
        pause(1.5)
        key("Q")
        pause(1.2)
        continue
    if quitting and not has(text, "Do you") and not read_map(text) and not has(text, "[C]ontinue") \
            and not has(text, "[P]lay again") and not has(text, "Press any key"):
        break
    c = choose(text)
    if c is None:
        # Nothing readable yet (or the game has ended)
        if not any(line.strip() for line in text):
            break
        time.sleep(0.2)
        if time.time() - start > PLAY_SECONDS + 30:
            break
        continue
    if read_map(text):
        in_game = True
    popup = has(text, "Press any key") or has(text, "[C]ontinue") or has(text, "[P]lay again")
    if popup:
        pause(1.4 if in_game else 0.9)
    elif not in_game:
        pause(0.2)
    for ch in c:
        if ch == "\n":
            pause(0.3)
            enter()
        else:
            key(ch)
            pause(0.15, 0.25)
    if has(text, "Press any key to exit"):
        break
    if in_game:
        pause(0.35, 0.55)

played = time.time() - start
sys.stderr.write("shown: %s, %.0f s\n" % (shown, played))
# A take that ends in an early death is too short to show the castle
sys.exit(0 if all(shown.values()) and played > PLAY_SECONDS - 5 else 1)
