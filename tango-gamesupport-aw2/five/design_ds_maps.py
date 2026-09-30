"""tangoAW2's Dual Strike maps (with the pack only; drawn into five/maps.txt by
design_maps.py): a Wasteland set and a sea set, one 2P, 3P, 4P and 5P map
each. Every layout is symmetric so each army starts alike: the 2P maps turn
about the centre (half a turn), the 3P maps are mirrored left to right with
army 3 on the middle line, the 4P and 5P maps are mirrored both ways
(Black Hole, army 5, in the middle of the 5P ones). Com Towers all start
neutral. Every army's ports reach every other army's by sea (a sea ring
round the map where the land would cut the water), every island has a beach
or port a Lander can use, and each army's Piperunner base touches its pipe
only through a seam, so breaking the seam ends its Piperunners there
(tools/aw2test/aw2test/traverse.py checks all of it). Legend in map.py."""

# Mirror images of the direction-dependent pieces.
FLIP_X = str.maketrans('WE12', 'EW21')
FLIP_Y = str.maketrans('NSnv1234', 'SNvn3412')
TURN = str.maketrans('NSWEnv12', 'SNEWvn21')


def half_turn(left, centre):
    """2P: `left` rows (x < n) and the centre column; the right half is the
    left half turned about the centre."""
    n, h = len(left[0]), len(left)
    assert all(len(r) == n for r in left) and len(centre) == h
    assert all(centre[y] == centre[h - 1 - y].translate(TURN) for y in range(h)), centre
    w = 2 * n + 1
    g = [list(left[y]) + [centre[y]] + [' '] * n for y in range(h)]
    for y in range(h):
        for x in range(n + 1, w):
            g[y][x] = g[h - 1 - y][w - 1 - x].translate(TURN)
    return g


def mirror_x(left, centre):
    """3P: `left` rows and the centre column, mirrored left to right."""
    n, h = len(left[0]), len(left)
    assert all(len(r) == n for r in left) and len(centre) == h
    return [list(left[y]) + [centre[y]] + [c.translate(FLIP_X) for c in left[y][::-1]] for y in range(h)]


def mirror_xy(q):
    """4P/5P: the top-left quadrant with the centre row and column (last row
    and column of `q`), mirrored both ways."""
    n = len(q) - 1
    assert all(len(r) == n + 1 for r in q)
    for c in q[n]:
        assert c == c.translate(FLIP_Y), ('centre row', c)
    for c in ''.join(r[n] for r in q):
        assert c == c.translate(FLIP_X), ('centre column', c)
    top = [list(r) + [c.translate(FLIP_X) for c in r[:n][::-1]] for r in q]
    return top + [[c.translate(FLIP_Y) for c in r] for r in top[:n][::-1]]


def draw(emit):
    # ==== The Wasteland set ==================================================
    ARMY = [1, 1, 2, 5, 6, 10, 9]

    # Rust Basin (2P, 25x17): a river down the middle with three bridges,
    # a sea ring joining the two bays, beaches; each side's pipe leaves its
    # base through a seam and runs along the coast towards the river.
    g = half_turn([
        "~~~~~~~~~~~~",
        "~~~~~~~.f.^^",
        "~~~r~~~P.f.c",
        "~~~~~~~.B...",
        "~,,,,,,RRRRR",
        "~^..f..R....",
        "~^C.^..R.c.f",
        "~...f..R..^.",
        "~..1RRRRRRRR",
        "~.f....R....",
        "~....C.R.t.c",
        "~..f...R.f..",
        "~.....BRRRRR",
        "~c..C.Z.A.^.",
        "~....fI.f.cX",
        "~,,^^.IIIII.",
        "~~~~~~~~~~~~",
    ], "~---=---=---=---~")
    emit('Rust Basin', {1: ARMY, 2: ARMY}, g, armies=2, tab=3, colours=(1, 2), look='wasteland')

    # Dune Fork (3P, 29x22): a river forks in the middle; Orange Star and
    # Blue Moon share the north, Green Earth holds the south; a sea ring
    # joins the three bays. A Com Tower between each pair of armies.
    g = mirror_x([
        "~~~~~~~~~~~~~~",
        "~~~r~~.f..^...",
        "~~~~~~P..^.c..",
        "~,,,,,..fC....",
        "~.....^.B....f",
        "~..1RRRRRRRRRR",
        "~.f....R...I..",
        "~.C....R..BZ..",
        "~...A.CR...Ic.",
        "~^f....R...I.^",
        "~...C..R.f.I..",
        "~------=------",
        "~.f....R....II",
        "~^...t.R.f....",
        "~.c....RRRRRRR",
        "~^...X.....C..",
        "~..f.....^..C.",
        "~.....f..c....",
        "~..^.......,,,",
        "~.........~~~~",
        "~...f.....~r~~",
        "~~~~~~~~~~~~~~",
    ], "~X.t.R------ZBR3BAP~~~")
    emit('Dune Fork', {1: ARMY, 2: ARMY, 3: ARMY}, g, armies=3, tab=5, colours=(1, 2, 3), look='wasteland')

    # Cinder Flats (4P, 29x29): four corners inside a sea ring; rivers run
    # from four bays towards a Black Obelisk ringed by Crystals and four Com
    # Towers.
    g = mirror_xy([
        "~~~~~~~~~~~~~~~",
        "~.f..^.c..~~~~~",
        "~.C.f....P~~r~~",
        "~f...B....~~~~~",
        "~...1RRR..,,,,-",
        "~.f....R.C...f-",
        "~..^...R.....^-",
        "~..A...RRRRRRR=",
        "~....fCR.^....-",
        "~.P....R..fB..-",
        "~~~~,C.R.IIZII-",
        "~~~~,..R.f....X",
        "~~~r,..R.c^.t..",
        "~~~~,f.R.....##",
        "~~~~---=---X.#O",
    ])
    emit('Cinder Flats', {a: ARMY for a in range(1, 5)}, g, armies=4, tab=6, colours=(1, 2, 3, 4), look='wasteland')

    # Black Wastes (5P, 29x29): four corners round Black Hole's fortress:
    # a Black Cannon facing north and one facing south, a Laser on each
    # side, four minicannons and four Crystals; Com Towers on the four axes,
    # lakes at the sides (no ports: they would not reach each other). Black
    # Hole's bases have pipes too.
    g = mirror_xy([
        ".f..^.c...-..f.",
        ".C......f.-.^..",
        "......B...-....",
        "...1RRRRRR=RRRR",
        "...R...R..-.c..",
        ".A.R.I.RC.-..^t",
        "...RBZ.R.f-....",
        "f..R.I.RRR=RRRR",
        ".C.R.I.R..-.^..",
        ".^.R.Z.R..f..##",
        ".C.R.I.R.c...#n",
        "~~,R.I.R...N.##",
        "~~,R...R.f..X..",
        "~~,R.c.R...IIZB",
        "r~,RRRRRtL...C5",
    ])
    emit('Black Wastes', {**{a: ARMY for a in range(1, 5)}, 5: [1, 1, 2, 5, 10, 9]}, g, look='wasteland')

    # ==== The sea set ============================================================
    NAVAL = [1, 1, 2, 6, 9, 23, 22]

    # Coral Strait (2P, 27x17): two islands across a strait, bridged to a
    # middle isle with two Com Towers (and one on each island), all inside a
    # sea ring; each side's two bases sit on a pipe through seams, and the
    # pipe runs through the sea to the middle isle.
    g = half_turn([
        "~~~~~~~~~~~~~",
        "~,,,,,,,~~~r~",
        "~.t..^..~I~~~",
        "~......RBZ~~~",
        "~.C....R.I~~~",
        "~..^...RBZ~~~",
        "~...f..R.II.c",
        "~.^..f.R.~~f.",
        "~...1RRRR==RR",
        "~....R...~~.^",
        "~..A.RCf.~~..",
        "~.f..R..~~~~~",
        "~...CRRRP~~~~",
        "~.c....f~~~r~",
        "~..^..,,~~~~~",
        "~,,,,,~~~~~~~",
        "~~~~~~~~~~~~~",
    ], "~~~~~~.tRt.~~~~~~")
    emit('Coral Strait', {1: NAVAL, 2: NAVAL}, g, armies=2, tab=3, colours=(1, 2))

    # Trident Isles (3P, 29x20): three home islands round a middle isle with
    # three Com Towers and a beach north and south; each home island's pipe
    # leaves a base through a seam and runs out to the middle isle.
    g = mirror_x([
        "~~~~~~~~~~~~~~",
        "~~,,,,,,~~~r~~",
        "~.C..f..P~~~~~",
        "~..^...C.~~~,,",
        "~.A.1RRR.~~~.c",
        "~..f...RBZII..",
        "~.^..C.R.~~~f.",
        "~~.CBRRR.~~~..",
        "~~~,,,,~~~~~t.",
        "~~~~~~~~~~~~,,",
        "~~~r~~~~~~~~~~",
        "~~~~~~~~~~,,,,",
        "~~~~~~~~~.c..C",
        "~~~~~~~~~..f.R",
        "~~~~~~~~~.C.RR",
        "~~~~~~~~~^....",
        "~~~~~~~~~~.f..",
        "~~~~~~~~~~,...",
        "~~~~~~~~~~~,,,",
        "~~~~~~~~~~~~r~",
    ], "~~~,t.^.cIIIZBR3BAP~")
    emit('Trident Isles', {1: NAVAL, 2: NAVAL, 3: NAVAL}, g, armies=3, tab=5, colours=(1, 2, 3))

    # Harbor Cross (4P, 29x29): four corner islands inside a sea ring; each
    # one's pipe leaves a base through a seam and runs across the channel to
    # the middle isle, its four Com Towers and its four beaches.
    g = mirror_xy([
        "~~~~~~~~~~~~~~~",
        "~.f...^..~~~~~~",
        "~.C..f...~~r~~~",
        "~......C.~~~~~~",
        "~.A.1RRRP~~~~~~",
        "~...R..R.~~~~~~",
        "~.^.R..R^~~~~~~",
        "~.f.B.CR.~~~~~~",
        "~,,,,,,RBZII~~~",
        "~~~~~~~~~~~I~~~",
        "~~~r~~~~~~~I~~,",
        "~~~~~~~~~~~.c.R",
        "~~~~~~~~~~~ft.R",
        "~~~~~~~~~~~..^R",
        "~~~~~r~~~~,RRRR",
    ])
    emit('Harbor Cross', {a: NAVAL for a in range(1, 5)}, g, armies=4, tab=6, colours=(1, 2, 3, 4))

    # Coral Crown (5P, 29x29): four corner islands and Black Hole's middle
    # island; a Com Tower on each of the four islets between them, reached
    # by the corner islands' pipes. Black Hole's bases have pipes too.
    g = mirror_xy([
        "..f...^..~~~~~~",
        ".C.....C.~~~...",
        "...^....BZII.ct",
        ".A.1RRRRR~~~..f",
        "...R..f..~~~,,,",
        ".f.R....P~~~~~~",
        "...R.C...~~r~~~",
        ".^.R.....~~~~~~",
        "..BR..f..~~~~~~",
        "~~Z~~~~~~~~~~~~",
        "~~I~~~~~~~~..fP",
        "~~I~~~~~~~.f...",
        ".f...~~~~~.I..C",
        "c...,~~~~~.I.RR",
        "..t.,~~r~~.ZBR5",
    ])
    emit('Coral Crown', {**{a: NAVAL for a in range(1, 5)}, 5: [1, 1, 2, 5, 9, 23]}, g)
