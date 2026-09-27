# tangoAW2

Rollback netplay for **Advance Wars 2: Black Hole Rising** (GBA, USA), with
everything unlocked and Black Hole as a fifth playable army.

tangoAW2 is a fork of [Tango](https://github.com/tangobattle/tango), the
rollback netplay client for Mega Man Battle Network, by the Tango
developers. It keeps Tango's emulator (mGBA), rollback engine, lobby and
networking, and adds Advance Wars 2 on top.

No game is included. You need your own copy of the cartridge dumped to a
`.gba` file.

## What it does

- **Rollback netplay.** Both players run the same emulated Game Boy
  Advance in the game's own Versus mode. Your button presses show up
  instantly. If your friend's input arrives late, the game quietly
  rewinds a few frames and replays them with the right input.
- **Turns are enforced.** On the battlefield only the player whose army
  is moving can press anything. In menus (map select, CO select, rules,
  the "Next turn" screen) both players can press. With fog of war on,
  the waiting player's screen goes dark during the other army's turn.
- **Everything unlocked.** Every CO including Sturm, every CO colour
  edit, every Battle Map, Hard Campaign and the Sound Room. This is
  applied while the game runs, like Slippi's codes for Melee, so the ROM
  file is never modified. Save in-game and it sticks.
- **Five armies.** Before a match you pick the armies, for example
  "Black Hole vs Orange Star", online in the lobby and offline beside
  Play offline. The choices are Orange Star, Blue Moon,
  Yellow Comet, Green Earth and Black Hole. Units, buildings and
  banners take that army's colours.

## Get it

Download the latest build for your system from
[Releases](https://github.com/rkho4642/tangoAW2/releases):

| System | File |
| --- | --- |
| Windows 10/11 | `tangoaw2-x86_64-windows.exe` (installer) |
| macOS (Apple Silicon and Intel) | `tangoaw2-macos.dmg` |
| Linux | `tangoaw2-x86_64-linux.AppImage` |

On macOS the app is not signed. The first time, right-click it and choose
Open.

## First run

1. Open tangoAW2 and pick a nickname.
2. Put your Advance Wars 2 file in the `roms` folder the welcome screen
   shows. It must be the USA cartridge (No-Intro
   `Advance Wars 2 - Black Hole Rising (USA)`, CRC32 `5AD0E571`).
3. That's it. tangoAW2 selects the game and creates a blank save for you
   the first time. Everything is unlocked anyway.

## Playing alone

Press **Play offline** on the Play tab. The game starts straight away, with
everything unlocked, and needs no internet connection. Campaign, War Room,
Versus against the computer and the Design Room all work.

The picker beside Play offline sets the armies for offline Versus, for
example "Black Hole vs Orange Star", or "Map colours" for each map's own.
Campaign and War Room always keep their story armies.

## Playing with a friend

Your friend installs tangoAW2 the same way, with their own copy of the
same cartridge. Both of you need the same tangoAW2 version.

**Link code (easiest).** Both of you type the same made-up code, such as
`sturm-4812`, into the link-code box on the Play tab. The matchmaking
server introduces the two apps and then gets out of the way. It works
through most home routers without any setup.

**Direct connection (no server).** One player types `/host` and the
other types `/connect <host's IP address>`. The host must be reachable on
UDP port 24680. That works on the same network, over a VPN such as
Tailscale, or with that port forwarded on the host's router.

Then:

1. Pick the armies in the lobby. Both players see the same choice.
2. Both press Ready. The game starts from power-on for both of you.
3. Go to **Versus → New**, pick a map, and on the **Teams** screen set
   army 2 to **2P** (move right to the "CP" marker and press up).
4. Pick COs and rules. With fog of war on, the screen goes dark for the
   waiting player during the other army's turn, so nobody sees through
   the other side's fog.
5. Play. Player 1 moves army 1 (and army 3 on four-army maps); player 2
   moves army 2 (and army 4).

The army icons on the Teams screen still show the map's usual colours.
The colours you picked take over once the battle starts.

## Running two copies on one computer

For testing, start two copies with separate profiles, then `/host` in
one and `/connect 127.0.0.1` in the other:

```sh
TANGOAW2_PROFILE=~/tangoaw2-p1 ./tango
TANGOAW2_PROFILE=~/tangoaw2-p2 ./tango
```

Each profile keeps its own config, saves and `roms` folder.
`TANGOAW2_AUTOSTART=offline` presses Play offline as soon as the library
is scanned, which is handy for scripted checks.

## Building from source

Install Rust stable, CMake, Ninja and `protoc`, then:

```sh
cargo run --release --bin tango
```

The tools behind the Advance Wars 2 work are included:

```sh
# Drive the game headlessly from a script: screenshots, RAM dumps, pokes.
cargo run --release -p tango-backend-mgba --example gba_probe -- rom.gba script.txt

# Two rollback peers with a delayed, jittery fake network; checks both
# end on the same frame as a replay of the confirmed inputs.
cargo run --release -p tango-gamesupport-aw2 --example aw2_rollback_sim -- rom.gba out/
```

How the Advance Wars 2 support works, and the RAM addresses it relies
on, is in [docs/AW2.md](docs/AW2.md). Tango's own documentation is in
[docs/TANGO_UPSTREAM_README.md](docs/TANGO_UPSTREAM_README.md),
[ARCHITECTURE.md](ARCHITECTURE.md) and [CONTRIBUTING.md](CONTRIBUTING.md).

## License

GPL-3.0-or-later, like Tango. See [LICENSE](LICENSE) and
[CREDITS.md](CREDITS.md). Advance Wars is a trademark of Nintendo.
tangoAW2 is not affiliated with Nintendo or Intelligent Systems.
