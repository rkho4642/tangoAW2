//! Dual Strike's map looks (Wasteland, Desert, Snow), with the Dual Strike
//! pack.
//!
//! In Dual Strike a map's look is one of its tilesets: the same terrain,
//! drawn in Wasteland's dry colours (tan plains, olive scrub, rocky pink
//! mountains, a red sea), Desert's sand or Snow's white. It changes no
//! rules. Here Wasteland is a per-map setting of design maps (and the look
//! of tangoAW2's Wasteland Versus maps); Survival's maps bring their own
//! look ([`set_ds_look`]).
//!
//! - Drawing: with a Dual Strike look the map is drawn with Dual Strike's
//!   own terrain graphics, converted from the pack once ([`crate::ds_look`]:
//!   tiles, the sea's and river's frames, metatiles, colours) and placed in
//!   the ROM image's free space ([`look_rom`]). The game reads its terrain
//!   through nine literal-pool words ([`sync`]: the tiles
//!   `LoadGameplayGraphics` decompresses, the metatiles `BlitMapRow` and
//!   `BlitMapColumn` draw from, the frames `LoadSeaAnimFrame` and
//!   `LoadRiverAnimFrame` copy); they point at the look's data while one is
//!   drawn and at AW2's otherwise, set from RAM alone at each reader's entry
//!   and every frame, so the drawing is the same on every console and after
//!   any rollback. While a look is drawn, `BlitMapRow` and `BlitMapColumn`
//!   are done here ([`Painter`]): each mountain is the one Dual Strike draws
//!   at its position, and the peak or treetops of a mountain or wood go over
//!   the cell above, as composite tiles made in the look's free tiles.
//! - Colours: AW2 draws its terrain with BG palettes 0-3 (and 4-7, the
//!   same darkened for fog), one set per weather (`0x0849BD20`, loaded by
//!   `sub_08035020`). A look's clear set is its converted palettes, its fog
//!   half Dual Strike's fog colours ([`crate::ds_look::Look::fog`]); as in
//!   Dual Strike, weather changes no colour: the rain, snow and sandstorm
//!   sets are the clear set (checked against melonDS frames).
//! - Storage: the last byte of the design-map record (+0x723, after the
//!   unit cells), as [`MAGIC`] | biome, saved with the map
//!   ([`crate::design5::save_record`]'s trap) and read when a design map is
//!   loaded for the editor or a battle. In play it is kept in the high bits
//!   of [`crate::sandstorm::STATE`] (saved with a suspended game).
//! - The Design Room switches Normal and Wasteland ([`toggle`]); its map is
//!   redrawn in the new look at once ([`editor_tick`]).

use mgba::core::Core;
use std::sync::OnceLock;

use crate::ds_weather::is_on;
use crate::sandstorm::STATE;

const BIOME_SHIFT: u8 = 4;
const BIOME_BITS: u8 = 0x70;
pub const NORMAL: u8 = 0;
pub const WASTELAND: u8 = 1;
pub const DESERT: u8 = 2;
pub const SNOW: u8 = 3;
/// Means to an End's own look: Dual Strike draws its map (0xF8) with
/// palette `bmap/00b` (arm9 `0x020F92E4`: map 0xF8 -> `0x0216A1DC` "00b")
/// in place of its look's (Wasteland's, `bmap/009`): Desert's colours but
/// for its terrain palette 2 (the Grand Bolt's greys).
pub const GRAND_BOLT_LOOK: u8 = 4;

/// The record's biome byte: [`MAGIC`] | biome, anything else Normal (maps
/// saved before 0.3.0 hold whatever the game left there).
const RECORD_BIOME: u32 = 0x723;
const MAGIC: u8 = 0xB0;
const MAGIC_BITS: u8 = 0xF8;

/// The map being played (design maps are 0xB4..0xBF).
const MAP_ID: u32 = 0x0300_3FC2;

pub fn biome(core: &Core) -> u8 {
    (core.raw_read_8(STATE, -1) & BIOME_BITS) >> BIOME_SHIFT
}

pub fn set_biome(core: &mut Core, b: u8) {
    let s = core.raw_read_8(STATE, -1);
    let want = (s & !BIOME_BITS) | ((b << BIOME_SHIFT) & BIOME_BITS);
    if want != s {
        core.raw_write_8(STATE, -1, want);
    }
}

/// Whether the map is drawn as Wasteland now.
pub fn is_wasteland(core: &Core) -> bool {
    drawn(core) == Some(WASTELAND)
}

/// At every map start: tangoAW2's Wasteland maps are Wasteland
/// ([`crate::five_map::is_wasteland_map`]); any other map that is not a
/// design map (0xB4..0xB7, their biome is read from their record) is Normal.
pub fn map_start(core: &mut Core) {
    let id = core.raw_read_8(MAP_ID, -1);
    let want = if crate::five_map::is_wasteland_map(id) {
        WASTELAND
    } else if (0xB4..=0xB7).contains(&id) {
        return;
    } else {
        NORMAL
    };
    if biome(core) != want {
        set_biome(core, want);
    }
}

/// `sub_0803D2F8` (a design-map record, r1, laid out for the editor or a
/// battle): its biome.
pub const LOAD_RECORD: u32 = 0x0803_D2F8;
pub fn load_record(core: &mut Core) {
    let record = core.gba().cpu().gpr(1) as u32;
    let b = core.raw_read_8(record + RECORD_BIOME, -1);
    let biome = if b & MAGIC_BITS == MAGIC { b & 7 } else { NORMAL };
    if biome != NORMAL || self::biome(core) != NORMAL {
        set_biome(core, biome);
    }
}

/// A design map about to be saved (the record at `record`): its biome goes
/// with it, with the pack on (a map saved without it keeps the byte as the
/// game left it).
pub fn save_record(core: &mut Core, record: u32) {
    if !is_on(core) {
        return;
    }
    let b = biome(core);
    let at = record + RECORD_BIOME;
    let now = core.raw_read_8(at, -1);
    if b != NORMAL {
        core.raw_write_8(at, -1, MAGIC | b);
    } else if now & MAGIC_BITS == MAGIC {
        core.raw_write_8(at, -1, 0);
    }
}

// --- The looks -----------------------------------------------------------

/// AW2's terrain: tiles (LZ77), metatiles, tile classes and the clear,
/// snow and rain colour sets (128 each: palettes 0-3, then 4-7 for fog).
const AW2_TILES: u32 = 0x080B_D1EC;
const AW2_METATILES: u32 = 0x080B_FBC4;
const AW2_CLASSES: u32 = 0x080C_1BC4;
const AW2_SEA: u32 = 0x080C_1FC4;
const AW2_RIVER: u32 = 0x080C_9FC4;
const CLEAR: u32 = 0x080B_F8C4;
const SNOW_SET: u32 = 0x080B_FAC4;
const RAIN: u32 = 0x080B_F9C4;

/// Each look's source in the pack ([`crate::ds_look`]).
const SOURCES: [(u8, crate::ds_look::Source); 4] = [
    (WASTELAND, crate::ds_look::Source { tiles: "bmap/001", palette: "bmap/009" }),
    (DESERT, crate::ds_look::Source { tiles: "bmap/001", palette: "bmap/008" }),
    (SNOW, crate::ds_look::Source { tiles: "bmap/000", palette: "bmap/00a" }),
    (GRAND_BOLT_LOOK, crate::ds_look::Source { tiles: "bmap/001", palette: "bmap/00b" }),
];

/// AW2's own fog relation, fitted over its 60 terrain colours (x256, per
/// 5-bit channel): rows are the clear colour's r, g, b and 1, columns the
/// result's r, g, b. Only for a colour no Dual Strike tile has.
const FOG: [[i32; 3]; 4] = [[136, 22, 14], [40, 156, 101], [19, 4, 117], [-305, -2, -335]];

fn apply(m: &[[i32; 3]; 4], c: u16) -> u16 {
    let v = [(c & 31) as i32, ((c >> 5) & 31) as i32, ((c >> 10) & 31) as i32];
    let mut out = 0u16;
    for j in 0..3 {
        let x = v[0] * m[0][j] + v[1] * m[1][j] + v[2] * m[2][j] + m[3][j];
        let x = ((x + 128).div_euclid(256)).clamp(0, 31) as u16;
        out |= x << (5 * j);
    }
    out
}

fn u16s(b: &[u8]) -> Vec<u16> {
    b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
}

fn bytes(v: &[u16]) -> Vec<u8> {
    v.iter().flat_map(|c| c.to_le_bytes()).collect()
}

/// A look's colour sets: clear, rain, snow (weather) and sandstorm, 128
/// colours each as AW2's.
pub struct Colours {
    pub clear: Vec<u16>,
    pub rain: Vec<u16>,
    pub snow: Vec<u16>,
    pub sand: Vec<u16>,
}

pub struct Built {
    pub look: crate::ds_look::Look,
    pub colours: Colours,
}

/// The looks, by biome (index biome - 1), worked out once.
static LOOKS: OnceLock<Vec<Option<Built>>> = OnceLock::new();

pub fn look(b: u8) -> Option<&'static Built> {
    LOOKS.get()?.get((b as usize).checked_sub(1)?)?.as_ref()
}

/// The terrain as AW2's ROM image has it.
pub fn aw2_terrain(core: &Core) -> Option<crate::ds_look::Aw2> {
    let mut packed = vec![0u8; 0x8000];
    core.raw_read_range(AW2_TILES, -1, &mut packed);
    let tiles = crate::ds_art::lz10(&packed)?;
    let mut mt = vec![0u8; 8 * crate::ds_look::METATILES];
    core.raw_read_range(AW2_METATILES, -1, &mut mt);
    let mut classes = vec![0u8; crate::ds_look::METATILES];
    core.raw_read_range(AW2_CLASSES, -1, &mut classes);
    Some(crate::ds_look::Aw2 { tiles, metatiles: u16s(&mt), clear: read_set(core, CLEAR), classes })
}

fn read_set(core: &Core, at: u32) -> Vec<u16> {
    let mut b = vec![0u8; 256];
    core.raw_read_range(at, -1, &mut b);
    u16s(&b)
}

/// One look, worked out from AW2's ROM image and the pack.
pub fn derive(core: &Core, b: u8) -> Option<Built> {
    let (_, src) = SOURCES.iter().find(|(x, _)| *x == b)?;
    let aw2 = aw2_terrain(core)?;
    let files = crate::ds_look::from_pack(src)?;
    let look = crate::ds_look::build(&aw2, &files.pack())?;
    let (mut clear, mut rain, mut snow) = (aw2.clear, read_set(core, RAIN), read_set(core, SNOW_SET));
    for i in 0..64 {
        if i % 16 == 0 {
            continue;
        }
        let c = look.colours[i];
        // Dual Strike's own fog colour (its fog sub-palette), AW2's
        // relation for a colour no Dual Strike tile has.
        let f = match look.fog[i] {
            crate::ds_look::NO_FOG => apply(&FOG, c),
            f => f,
        };
        // Weather changes no colour in Dual Strike (rain and snow fall,
        // sand blows over the map: AW2's particles, crate::sandstorm's).
        for set in [&mut clear, &mut rain, &mut snow] {
            set[i] = c;
            set[64 + i] = f;
        }
    }
    let sand = clear.clone();
    Some(Built { look, colours: Colours { clear, rain, snow, sand } })
}

/// Each look's data in the ROM image's free space (past the music):
/// metatiles, tiles (LZ77), sea frames, river frames, the clear, rain,
/// snow and sandstorm sets.
const LOOK_DATA: u32 = 0x08E8_0000;
const LOOK_SIZE: u32 = 0x2_0000;
const AT_METATILES: u32 = 0;
const AT_TILES: u32 = 0x2000;
const AT_SEA: u32 = 0x9000;
const AT_RIVER: u32 = 0x1_1000;
const AT_CLEAR: u32 = 0x1_7000;
const AT_RAIN: u32 = AT_CLEAR + 0x100;
const AT_SNOW: u32 = AT_CLEAR + 0x200;
const AT_SAND: u32 = AT_CLEAR + 0x300;
/// How many free tiles the look has for composites (for tests and the doc).
const AT_POOL: u32 = 0x1_7FF8;
const AT_SENTINEL: u32 = 0x1_7FFC;
const LOOK_MAGIC: u32 = 0x4B4C_5344; // "DSLK"

pub fn look_rom(b: u8) -> u32 {
    LOOK_DATA + LOOK_SIZE * (b as u32 - 1)
}

/// LZ77 of literals only (decompresses anywhere, VRAM included).
fn lz_literal(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x10, data.len() as u8, (data.len() >> 8) as u8, (data.len() >> 16) as u8];
    for chunk in data.chunks(8) {
        out.push(0);
        out.extend_from_slice(chunk);
    }
    out
}

/// tangoAW2's Design Room bar name in the ROM image's free space.
const DATA: u32 = 0x0867_1000;
/// The Design Room bar entry's name ([`WASTE_NAME`]).
pub const NAME_AT: u32 = DATA + 0x300;
const DATA_SENTINEL: u32 = DATA + 0xFFC;
const DATA_MAGIC: u32 = 0x3357_5344; // "DSW3"

/// Every frame with the pack on: the looks in the ROM image (worked out
/// once), and the game's terrain pointers the look's.
pub fn tick(core: &mut Core, on: bool) {
    if on {
        if LOOKS.get().is_none() {
            let looks = SOURCES.iter().map(|(b, _)| derive(core, *b)).collect();
            let _ = LOOKS.set(looks);
        }
        for (b, _) in SOURCES {
            let Some(l) = look(b) else { continue };
            let base = look_rom(b);
            if core.raw_read_32(base + AT_SENTINEL, -1) == LOOK_MAGIC {
                continue;
            }
            core.raw_write_range(base + AT_METATILES, -1, &bytes(&l.look.metatiles));
            core.raw_write_range(base + AT_TILES, -1, &lz_literal(&l.look.tiles));
            core.raw_write_range(base + AT_SEA, -1, &l.look.sea);
            core.raw_write_range(base + AT_RIVER, -1, &l.look.river);
            let c = &l.colours;
            for (at, set) in [(AT_CLEAR, &c.clear), (AT_RAIN, &c.rain), (AT_SNOW, &c.snow), (AT_SAND, &c.sand)] {
                core.raw_write_range(base + at, -1, &bytes(set));
            }
            core.raw_write_32(base + AT_POOL, -1, l.look.pool.len() as u32);
            core.raw_write_32(base + AT_SENTINEL, -1, LOOK_MAGIC);
        }
        if core.raw_read_32(DATA_SENTINEL, -1) != DATA_MAGIC {
            core.raw_write_range(NAME_AT, -1, &WASTE_NAME);
            core.raw_write_32(DATA_SENTINEL, -1, DATA_MAGIC);
        }
    }
    sync(core);
}

/// The look drawn now, if not AW2's own.
pub fn drawn(core: &Core) -> Option<u8> {
    let b = biome(core);
    (is_on(core) && b != NORMAL && look(b).is_some()).then_some(b)
}

/// The colour set for the map's weather (`sub_08035020`), when the look
/// is a Dual Strike one: 0 clear, 1 snow, 2 rain; or the sandstorm's.
pub fn palette_set(core: &Core, sand: bool, weather: u8) -> Option<u32> {
    let base = look_rom(drawn(core)?);
    Some(base
        + match (sand, weather) {
            (true, _) => AT_SAND,
            (false, 1) => AT_SNOW,
            (false, 2) => AT_RAIN,
            _ => AT_CLEAR,
        })
}

/// The literal-pool words through which the game reads its terrain:
/// (where, AW2's value, the look's offset).
const POOLS: [(u32, u32, u32); 9] = [
    // LoadGameplayGraphics: the tiles.
    (0x0802_34D8, AW2_TILES, AT_TILES),
    // BlitMapColumn and BlitMapRow: the metatiles (and +6).
    (0x0802_3B08, AW2_METATILES, AT_METATILES),
    (0x0802_3C6C, AW2_METATILES, AT_METATILES),
    (0x0802_3B1C, AW2_METATILES + 6, AT_METATILES + 6),
    (0x0802_3BA8, AW2_METATILES + 6, AT_METATILES + 6),
    (0x0802_3C80, AW2_METATILES + 6, AT_METATILES + 6),
    (0x0802_3D10, AW2_METATILES + 6, AT_METATILES + 6),
    // LoadSeaAnimFrame, LoadRiverAnimFrame: the frames.
    (0x0802_1D94, AW2_SEA, AT_SEA),
    (0x0802_1DCC, AW2_RIVER, AT_RIVER),
];

/// The terrain pointers follow the look (AW2's own whenever no Dual
/// Strike look is drawn, so with the pack off nothing changes). A function
/// of RAM alone, run before anything reads them: every frame and at each
/// reader's entry ([`traps`]).
pub fn sync(core: &mut Core) {
    let base = drawn(core).map(look_rom);
    for (at, aw2, off) in POOLS {
        let want = base.map_or(aw2, |b| b + off);
        if core.raw_read_32(at, -1) != want {
            core.raw_write_32(at, -1, want);
        }
    }
}

const LOAD_GRAPHICS: u32 = 0x0802_3360;
const BLIT_COLUMN: u32 = 0x0802_3A4C;
const BLIT_ROW: u32 = 0x0802_3BAC;
const SEA_FRAME: u32 = 0x0802_1D64;
const RIVER_FRAME: u32 = 0x0802_1DA0;

// --- The Design Room ---------------------------------------------------------

/// The game's working palettes (copied to palette RAM every frame).
const PAL_BUFFER: u32 = 0x0300_20C0;
const MAP_COLOURS: usize = 128;
/// Terrain tiles in VRAM (BG3's characters).
const VRAM_TILES: u32 = 0x0600_8000;
/// The map (gMap): scroll at +4/+6, camera origin at +0xC/+0xE, tiles at
/// +0xA22, seen cells at +0x234A, row offsets at +0x417A.
const MAP: u32 = 0x0201_E450;
/// Points at BG3's tilemap buffer.
const BG3_BUFFER_POINTER: u32 = 0x0849_9584;
const BG3CNT: u32 = 0x0400_000E;

/// A on the map with the bar's Wasteland entry
/// ([`crate::design_bar::WASTE_WORD`]): the map switches between Normal and
/// Wasteland.
pub fn toggle(core: &mut Core) {
    let b = if biome(core) == WASTELAND { NORMAL } else { WASTELAND };
    set_biome(core, b);
}

/// Tiles 0..256: never animated.
const SAMPLE: usize = 256;

static AW2_TILE_DATA: OnceLock<Option<Vec<u8>>> = OnceLock::new();

fn aw2_tiles(core: &Core) -> Option<&'static Vec<u8>> {
    AW2_TILE_DATA
        .get_or_init(|| {
            let mut packed = vec![0u8; 0x8000];
            core.raw_read_range(AW2_TILES, -1, &mut packed);
            crate::ds_art::lz10(&packed)
        })
        .as_ref()
}

/// Draws cells into BG3's tilemap buffer: from the look when one is drawn,
/// else from AW2's metatiles. The composites (a cell's bottom quadrants with
/// the upper part of the cell below over them, [`crate::ds_look::Look::composite`])
/// go into the look's pool of free tiles in VRAM: one already there is used
/// again, else the first pool tile no tilemap entry uses. All it keeps is in
/// VRAM and the tilemap buffer, so it is the same on every console and
/// after a rollback.
struct Painter<'a> {
    look: Option<&'a crate::ds_look::Look>,
    buf: u32,
    vram: Vec<u8>,
    used: Vec<bool>,
    loaded: bool,
    bolt_slots: Option<Vec<usize>>,
}

const TILE: usize = 32;

impl<'a> Painter<'a> {
    fn new(core: &Core, look: Option<&'a crate::ds_look::Look>) -> Self {
        Painter { look, buf: core.raw_read_32(BG3_BUFFER_POINTER, -1), vram: Vec::new(), used: Vec::new(), loaded: false, bolt_slots: None }
    }

    fn load(&mut self, core: &Core) {
        if self.loaded {
            return;
        }
        self.vram = vec![0u8; TILE * crate::ds_look::TILES];
        core.raw_read_range(VRAM_TILES, -1, &mut self.vram);
        let mut map = vec![0u8; 0x800];
        core.raw_read_range(self.buf, -1, &mut map);
        self.used = vec![false; 0x400];
        for e in u16s(&map) {
            self.used[(e & 0x3FF) as usize] = true;
        }
        self.loaded = true;
    }

    fn composite(&mut self, core: &mut Core, l: &crate::ds_look::Look, e: u16, o: u8, side: u8) -> u16 {
        let Some((tile, b)) = l.composite(e, o, side) else {
            return e;
        };
        match self.pool_tile(core, l, &tile) {
            Some(s) => s as u16 | b << 12,
            // The pool is full: the cell without the upper part.
            None => e,
        }
    }

    /// A tile of the look's pool holding `tile`: one already there, else
    /// the first one no tilemap entry uses (written to VRAM); None when the
    /// pool is full.
    fn pool_tile(&mut self, core: &mut Core, l: &crate::ds_look::Look, tile: &[u8]) -> Option<usize> {
        let pool = l.pool.clone();
        self.tile_in(core, &pool, tile)
    }

    /// [`Self::pool_tile`] over the slots `pool`.
    fn tile_in(&mut self, core: &mut Core, pool: &[usize], tile: &[u8]) -> Option<usize> {
        self.load(core);
        let at = |s: usize| TILE * s..TILE * s + TILE;
        let s = match pool.iter().copied().find(|&s| self.vram[at(s)] == *tile) {
            Some(s) => s,
            None => {
                let s = pool.iter().copied().find(|&s| !self.used[s])?;
                self.vram[at(s)].copy_from_slice(tile);
                core.raw_write_range(VRAM_TILES + (TILE * s) as u32, -1, tile);
                s
            }
        };
        self.used[s] = true;
        Some(s)
    }

    /// Where the Grand Bolt's tiles go: the look's pool, then the static
    /// tiles (not the sea's or river's, which their animation rewrites) no
    /// other cell of the map draws with.
    fn bolt_slots(&mut self, core: &Core, l: &crate::ds_look::Look) -> Vec<usize> {
        if let Some(s) = &self.bolt_slots {
            return s.clone();
        }
        let (w, h) = (core.raw_read_16(MAP, -1) as u32, core.raw_read_16(MAP + 2, -1) as u32);
        let mut drawn = vec![false; crate::ds_look::TILES];
        for y in 0..h.min(64) {
            for x in 0..w.min(64) {
                if crate::grand_bolt::cell_at(core, x, y).is_some() {
                    continue;
                }
                let row = core.raw_read_16(MAP + 0x417A + 2 * y, -1) as u32;
                let t = core.raw_read_16(MAP + 0xA22 + 2 * (row + x), -1);
                for e in l.entries(t, x, y) {
                    if let Some(d) = drawn.get_mut((e & 0x3FF) as usize) {
                        *d = true;
                    }
                }
            }
        }
        let mut slots = l.pool.clone();
        for t in (1..0x100).chain(0x260..crate::ds_look::TILES) {
            if !drawn[t] && !slots.contains(&t) {
                slots.push(t);
            }
        }
        self.bolt_slots = Some(slots.clone());
        slots
    }

    /// A cell's four tilemap entries as `BlitMapRow`/`BlitMapColumn` draw
    /// them (fog: palette + 4).
    fn entries(&mut self, core: &mut Core, x: u32, y: u32) -> [u16; 4] {
        let row = |core: &Core, y: u32| core.raw_read_16(MAP + 0x417A + 2 * y, -1) as u32;
        let cell = row(core, y) + x;
        let fog = if core.raw_read_8(MAP + 0x234A + cell, -1) == 0 { 0x4000 } else { 0 };
        let t = core.raw_read_16(MAP + 0xA22 + 2 * cell, -1);
        // Means to an End's Grand Bolt: its own picture (crate::grand_bolt),
        // its tiles in the pool as the composites are, in its palette.
        if let (Some(l), Some(c), Some(b)) = (self.look, crate::grand_bolt::cell_at(core, x, y), crate::grand_bolt::bolt()) {
            let mut q = [0u16; 4];
            let mut whole = true;
            let slots = self.bolt_slots(core, l);
            for (k, e) in q.iter_mut().enumerate() {
                let (tile, flips) = b.tile(c, k);
                match self.tile_in(core, &slots, &tile) {
                    Some(s) => *e = s as u16 | flips | crate::grand_bolt::PALETTE << 12,
                    None => whole = false,
                }
            }
            if whole {
                return q.map(|v| v.wrapping_add(fog));
            }
        }
        let q = match self.look {
            Some(l) => {
                let mut q = l.entries(t, x, y);
                let (w, h) = (core.raw_read_16(MAP, -1) as u32, core.raw_read_16(MAP + 2, -1) as u32);
                if x < w && y + 1 < h {
                    let tb = core.raw_read_16(MAP + 0xA22 + 2 * (row(core, y + 1) + x), -1);
                    if let Some(o) = l.upper_at(tb, x, y + 1) {
                        for side in 0..2u8 {
                            q[2 + side as usize] = self.composite(core, l, q[2 + side as usize], o, side);
                        }
                    }
                }
                q
            }
            None => {
                let at = AW2_METATILES + 8 * t as u32;
                [0, 1, 2, 3].map(|k| core.raw_read_16(at + 2 * k, -1))
            }
        };
        q.map(|v| v.wrapping_add(fog))
    }

    fn put(&mut self, core: &mut Core, index: u32, q: [u16; 4]) {
        for (k, off) in [0u32, 1, 32, 33].into_iter().enumerate() {
            core.raw_write_16(self.buf + 2 * ((index + off) & 0x3FF), -1, q[k]);
            if self.loaded {
                self.used[(q[k] & 0x3FF) as usize] = true;
            }
        }
    }

    /// `BlitMapRow(a1, a2, x, y)` (`sub_08023BAC`): 16 cells from (x, y)
    /// into BG3's tilemap buffer at column a1, row a2.
    fn row(&mut self, core: &mut Core, a: [u32; 4]) {
        let mut col = (a[0] & 0xF) * 2;
        let line = (a[1] & 0xF) * 64;
        for i in 0..16 {
            let q = self.entries(core, a[2] + i, a[3]);
            self.put(core, line + col, q);
            col = (col + 2) & 0x1F;
        }
    }

    /// `BlitMapColumn(a1, a2, x, y)` (`sub_08023A4C`): 11 cells down from
    /// (x, y).
    fn column(&mut self, core: &mut Core, a: [u32; 4]) {
        let col = (a[0] & 0xF) * 2;
        let mut line = (a[1] & 0xF) * 64;
        for i in 0..=10 {
            let q = self.entries(core, a[2], a[3] + i);
            self.put(core, line + col, q);
            line = (line + 0x40) & 0x3FF;
        }
    }
}

fn args(core: &Core) -> [u32; 4] {
    let cpu = core.gba().cpu();
    [0, 1, 2, 3].map(|r| cpu.gpr(r) as u32 & 0xFFFF)
}

fn return_now(core: &mut Core) {
    let cpu = core.gba_mut().cpu_mut();
    let lr = cpu.gpr(14) as u32;
    cpu.set_thumb_pc(lr & !1);
}

/// At `BlitMapRow`'s and `BlitMapColumn`'s entry: with a Dual Strike look
/// the cells are drawn here (mountains by position and the upper parts
/// over the cells above need more than one metatile per tile id) and the
/// function returns at once; otherwise the game draws them.
fn blit_row(core: &mut Core) {
    sync(core);
    if let Some(l) = drawn(core).and_then(look) {
        let a = args(core);
        Painter::new(core, Some(&l.look)).row(core, a);
        return_now(core);
    }
}

fn blit_column(core: &mut Core) {
    sync(core);
    if let Some(l) = drawn(core).and_then(look) {
        let a = args(core);
        Painter::new(core, Some(&l.look)).column(core, a);
        return_now(core);
    }
}

/// `RenderMap` (`sub_08021D10`) done here: the visible cells drawn into
/// BG3's tilemap buffer, and the buffer copied to VRAM.
fn render_map(core: &mut Core) {
    let mut painter = Painter::new(core, drawn(core).and_then(look).map(|l| &l.look));
    let sx = (core.raw_read_16(MAP + 4, -1) as i16 >> 4) as i32;
    let sy = (core.raw_read_16(MAP + 6, -1) as i16 >> 4) as i32;
    let cx = core.raw_read_16(MAP + 0xC, -1) as i16 as i32;
    let cy = core.raw_read_16(MAP + 0xE, -1) as i16 as i32;
    for y in 0..16 {
        let a = [(sx - cx) as u16 as u32, (y + sy - cy) as u16 as u32, sx as u16 as u32, (y + sy) as u16 as u32];
        painter.row(core, a);
    }
    let buf = core.raw_read_32(BG3_BUFFER_POINTER, -1);
    let mut map = vec![0u8; 0x800];
    core.raw_read_range(buf, -1, &mut map);
    let block = (core.raw_read_16(BG3CNT, -1) as u32 >> 8) & 0x1F;
    core.raw_write_range(0x0600_0000 + 0x800 * block, -1, &map);
}

/// Every Design Room frame with the pack on: the map is drawn in its
/// look. Colours (palettes 0-7): only a set the editor loaded itself
/// (AW2's clear or a look's) is ever replaced, so nothing else drawn with
/// those palettes is touched. Tiles: when the terrain's tiles in VRAM are
/// another look's (AW2's or a Dual Strike one's), they are replaced and
/// the map redrawn.
pub fn editor_tick(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    sync(core);
    let drawn = drawn(core);
    // Colours.
    let mut now = vec![0u8; 2 * MAP_COLOURS];
    core.raw_read_range(PAL_BUFFER, -1, &mut now);
    let mut aw2 = vec![0u8; 2 * MAP_COLOURS];
    core.raw_read_range(CLEAR, -1, &mut aw2);
    let sets: Vec<Vec<u8>> = SOURCES.iter().filter_map(|(b, _)| look(*b)).map(|l| bytes(&l.colours.clear)).collect();
    let want = drawn.and_then(look).map_or(aw2.clone(), |l| bytes(&l.colours.clear));
    if now != want && (now == aw2 || sets.contains(&now)) {
        core.raw_write_range(PAL_BUFFER, -1, &want);
    }
    // Tiles: the first 256 (never animated), but for a look's pool, tell
    // the looks apart.
    let Some(aw2_tiles) = aw2_tiles(core) else {
        return;
    };
    let mut vram = vec![0u8; SAMPLE * TILE];
    core.raw_read_range(VRAM_TILES, -1, &mut vram);
    let is = |tiles: &[u8], pool: &[usize]| {
        (0..SAMPLE).all(|s| pool.contains(&s) || vram[TILE * s..TILE * s + TILE] == tiles[TILE * s..TILE * s + TILE])
    };
    let (want, pool): (&[u8], &[usize]) = match drawn.and_then(look) {
        Some(l) => (&l.look.tiles, &l.look.pool),
        None => (aw2_tiles, &[]),
    };
    if is(want, pool) {
        return;
    }
    let known = is(aw2_tiles, &[]) || SOURCES.iter().filter_map(|(b, _)| look(*b)).any(|l| is(&l.look.tiles, &l.look.pool));
    if known {
        core.raw_write_range(VRAM_TILES, -1, want);
        render_map(core);
    }
}

/// Set a design or Survival map's look from Dual Strike's look byte (0
/// normal, 1 snow, 2 desert, 3 wasteland), for maps brought over from
/// Dual Strike (Survival, the campaign).
pub fn set_ds_look(core: &mut Core, ds_look: u8) {
    let b = match ds_look {
        1 => SNOW,
        2 => DESERT,
        3 => WASTELAND,
        _ => NORMAL,
    };
    set_biome(core, b);
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (LOAD_RECORD, Box::new(load_record)),
        (LOAD_GRAPHICS, Box::new(sync)),
        (BLIT_COLUMN, Box::new(blit_column)),
        (BLIT_ROW, Box::new(blit_row)),
        (SEA_FRAME, Box::new(sync)),
        (RIVER_FRAME, Box::new(sync)),
    ]
}

/// "Waste", the Wasteland entry's name in the Design Room's terrain bar
/// (32x16, 4x2 tiles, the bar's font colours: 1 fill, 15 outline).
pub const WASTE_NAME: [u8; 256] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x1F, 0xF1, 0x00, 0x00,
    0x1F, 0xF1, 0xFF, 0xFF, 0x1F, 0xF1, 0x1F, 0xF1, 0x1F, 0xF1, 0x1F, 0xF1, 0x1F, 0xF1, 0x1F, 0xF1,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x1F, 0xF1, 0x00, 0x00,
    0x1F, 0xF1, 0x00, 0x00, 0x1F, 0xF1, 0x00, 0x00, 0x1F, 0xF1, 0x00, 0x00, 0x1F, 0xF1, 0xF0, 0xFF,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0xF0, 0x00, 0x00, 0x00, 0xF0, 0x00, 0x00, 0x00, 0xFF, 0x0F, 0xFF, 0xFF, 0x1F,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0x00, 0x00, 0x00, 0xF1, 0x00, 0x00, 0x00, 0xF1, 0x0F, 0x00, 0x00, 0x11, 0x0F, 0xFF, 0x0F,
    0x1F, 0xF1, 0x1F, 0xF1, 0x1F, 0xF1, 0x1F, 0xF1, 0xFF, 0x11, 0x11, 0x11, 0xF0, 0x1F, 0xF1, 0x1F,
    0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x1F, 0xF1, 0xFF, 0x11, 0x1F, 0xF1, 0x1F, 0x1F, 0x11, 0xFF, 0x1F, 0x1F, 0xF1, 0x0F, 0x1F, 0xF1,
    0xFF, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0x1F, 0xF1, 0xFF, 0xFF, 0xF1, 0xFF, 0xF0, 0xFF, 0xFF, 0xF1, 0xF0, 0xFF, 0x11, 0xFF, 0xF0,
    0xF0, 0xFF, 0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xF1, 0xFF, 0x1F, 0xFF, 0xF1, 0xF0, 0x11, 0xF1, 0xF1, 0xFF, 0xF1, 0xFF, 0x1F, 0xFF, 0x1F, 0xF1,
    0xFF, 0x0F, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fog_fit_and_literal_lz() {
        // White fogs to a light grey-blue; black stays dark.
        let white = 0x7FFF;
        let f = apply(&FOG, white);
        assert!((f & 31) < 31 && (f & 31) > 15);
        assert!(apply(&FOG, 0) & 31 <= 2);
        assert_eq!(lz_literal(&[1, 2, 3]), vec![0x10, 3, 0, 0, 0, 1, 2, 3]);
    }
}
