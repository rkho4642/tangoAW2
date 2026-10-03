# tangoAW2

Rollback netplay for **Advance Wars 2: Black Hole Rising** (GBA, USA).

A fork of [Tango](https://github.com/tangobattle/tango) (mGBA, rollback,
matchmaking) for Advance Wars 2. Both players run the real game in its own
Versus mode; your ROM is never modified (changes are applied in memory).
No game is included: you need your own dump of the USA cartridge.

<table>
<tr>
<td><img src="docs/screenshots/menu-title-tangoaw2.png" width="360" alt="The title screen with the tangoAW2 badge"></td>
<td><img src="docs/screenshots/black-hole-versus.png" width="360" alt="A Versus battle"></td>
</tr>
</table>

## Get it

Download the latest build from
[Releases](https://github.com/rkoh46/tangoAW2/releases/latest) (what
changed is in each release's notes): Windows (`.exe` installer), macOS on
Apple Silicon (`.dmg`, not signed: right-click the app and choose Open the
first time), Linux (`.AppImage`), or iPhone and iPad (`.ipa`, iOS 16 or
later: install it with [AltStore](https://altstore.io),
[SideStore](https://sidestore.io) or [Sideloadly](https://sideloadly.io);
it plays with on-screen buttons or a controller, and online with desktop
players).

## First run

1. Open tangoAW2 and pick a nickname.
2. Put your Advance Wars 2 ROM in the `roms` folder the welcome screen
   shows: No-Intro `Advance Wars 2 - Black Hole Rising (USA)`, CRC32
   `5AD0E571`.
   (on iPhone, import it from the welcome screen; the Dual Strike `.nds`
   goes there too).
3. Press **Play offline** to play alone, or see below to play a friend.

## Playing online

You both need tangoAW2 (same version) and your own copy of the ROM.

- **Link code:** both type the same made-up code (say `sturm-4812`) on the
  Play tab. Works through most home routers with no setup.
- **Direct:** one player types `/host`, the other `/connect <their IP>`
  (UDP port 24680 must be reachable: same network, a VPN such as
  Tailscale, or a forwarded port).

Both press Ready; the game starts from power-on for both. Go to
**Versus → New**, pick a map, and on the **Teams** screen set army 2 to
**2P**. Player 1 moves armies 1, 3 and 5; player 2 moves armies 2 and 4.

On the map only the moving army's player can press anything; menus are
shared. With fog on, the waiting player's screen is covered during the
other army's turn. The match runs on player 1's save (and design maps).

## What it adds

- **Everything unlocked:** all COs including Sturm, CO colour edits,
  Battle Maps, Hard Campaign and the Sound Room.
- **Black Hole as a Versus army** and **five-army maps** (a 5P Maps tab in
  Versus: press LEFT on the tab list), any army human or computer.
- **Design Room:** Black Hole as a fifth army and its inventions
  (minicannons, Laser, Black Cannons, Black Factory, Volcano, Deathray),
  which work in Versus as in the campaign.
- **An Advance Wars look** for the app, or your own background image.

With your own Advance Wars: Dual Strike (USA) `.nds` in the `roms` folder,
Dual Strike comes to AW2 (converted on your computer; without it nothing
changes, and online both players need it):

- **Units, COs and terrain:** its 7 units and 9 new COs with their
  pictures, animations, music and numbers; Com Towers, Sandstorm, its
  weather and fog looks, Wasteland, Desert and Snow terrain, the Black
  Crystal and Obelisk; eight new Versus maps.
- **Survival:** Money, Turn and Time, eleven maps each, with ranks and
  records.
- **DS Campaign:** all 28 missions on Dual Strike's world map, with its
  story, prologue, ending with a choice, credits and music; Hard campaign,
  mission records and saving mid-mission; missions open with Dual Strike's
  Setup phase. Two-front missions are fought on both fronts (look across
  with **Front**, **Send** units over; in Lightning Strikes and Ring of Fire
  turn **Intel → Auto CO** off to command the second yourself), and Means
  to an End has the real Grand Bolt.
- **CO skills:** Dual Strike's 40 skills, earned with EXP. **SELECT** on a
  CO screen (on Teams, on an army's CO) opens SET SKILLS; Versus' Rules
  screen turns them on (off by default).
- **CO tag pairs:** on Teams press **START** on a CO, then **UP**/**DOWN**
  for a partner. **Change** swaps COs; with both meters full **Tag** fires
  a Tag Power, with Dual Strike's pair boosts and screens.
  Sturm gets pairs of his own.

<table>
<tr>
<td><img src="docs/screenshots/ds-campaign.png" width="240" alt="The DS Campaign's first mission"></td>
<td><img src="docs/screenshots/ds-megatank-battle.png" width="240" alt="A Megatank firing in battle"></td>
<td><img src="docs/screenshots/ds-von-bolt-ex-machina.png" width="240" alt="Von Bolt's Ex Machina"></td>
</tr>
<tr>
<td><img src="docs/screenshots/ds-survival.png" width="240" alt="Turn Survival's first map"></td>
<td><img src="docs/screenshots/five-magma-crown.png" width="240" alt="A five-army map"></td>
<td><img src="docs/screenshots/design-inventions-editor.png" width="240" alt="Black Hole's inventions in the Design Room"></td>
</tr>
</table>

## Controls

Default keys: A/B = Z/X, L/R = A/S, START/SELECT = Enter/Space (change
them in Settings). On Versus' Teams screen **R** or **L** changes an
army's colour, Black Hole included (without Dual Strike, **SELECT** too).

Design Room: **R** opens the terrain bar (the inventions come after Silo),
**L** the unit bar, **SELECT** or **UP** changes army, **A** places. Save
with **SELECT → File → Save**; play it from **Versus → Design Maps**.

## Known limits

- USA cartridge only; two human players online (more armies can be
  computer-controlled).
- Five-army battles and Survival runs can't be suspended.
- The mini maps show Black Hole's buildings in neutral grey.
- DS Campaign: the tag screens are still pictures.

## How it's tested

Every change is played offline and online and checked by scripted runs of
the real game (`tools/aw2test`, with and without Dual Strike): rollback
peers over a jittery fake network must end byte-for-byte identical, games
without Dual Strike are compared byte for byte with the previous release,
and a test bot wins most DS missions through the pad. The checks are in
[CONTRIBUTING.md](CONTRIBUTING.md); how it all works is in
[docs/AW2.md](docs/AW2.md).

Most of the code and research was written with an AI coding assistant
(Claude) under the maintainer's direction. Bug reports, especially "this
doesn't play like the real game", are welcome on the tangoAW2 thread on
the Wars World News forums.

## Building from source

Install Rust stable, CMake, Ninja and `protoc`, then
`cargo run --release --bin tango`. To test netplay on one computer, start
two copies with `TANGOAW2_PROFILE` set to different folders, then `/host`
in one and `/connect 127.0.0.1` in the other. The iPhone/iPad build
(`ios/build.sh`, needs Xcode) is described in [ios/README.md](ios/README.md).

## License

GPL-3.0-or-later, like Tango. See [LICENSE](LICENSE) and
[CREDITS.md](CREDITS.md). Advance Wars is a trademark of Nintendo.
tangoAW2 is not affiliated with Nintendo or Intelligent Systems.
