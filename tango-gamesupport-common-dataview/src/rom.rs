use byteorder::ReadBytesExt;

/// A game's ROM-derived tables, behind the opaque
/// `tango_gamesupport::BoxedAssets` envelope. Nothing shared reads any
/// table: a game that models its ROM keeps the tables on its concrete
/// type and its own code downcasts through [`underlying_any`].
///
/// [`underlying_any`]: Assets::underlying_any
pub trait Assets: crate::save::AsAny {
    /// The game's own concrete assets, for game-specific code to
    /// downcast. A wrapper layered over another game's assets forwards
    /// to what it wraps; everything else answers with itself, which the
    /// default provides.
    fn underlying_any(&self) -> &dyn std::any::Any {
        self.as_any()
    }
}

/// The assets of a game that models nothing behind its save (netplay
/// only, no `load_rom_assets_fn`): no tables at all. Loading such a
/// save bakes from these, so the editor shell renders its empty state
/// instead of the game needing a no-assets path.
pub struct EmptyAssets;

impl Assets for EmptyAssets {}

#[repr(transparent)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Default, c2rust_bitfields::BitfieldStruct)]
pub struct Bgr555 {
    #[bitfield(name = "r", ty = "u8", bits = "0..=4")]
    #[bitfield(name = "g", ty = "u8", bits = "5..=9")]
    #[bitfield(name = "b", ty = "u8", bits = "10..=14")]
    raw: [u8; 2],
}

impl Bgr555 {
    pub const fn new(raw: [u8; 2]) -> Self {
        Self { raw }
    }

    pub const fn to_le(&self) -> u16 {
        u16::from_le_bytes(self.raw)
    }

    pub const fn to_rgba8(&self) -> image::Rgba<u8> {
        let raw = self.to_le();
        image::Rgba([
            ((raw & 0x1f) * 0xff / 0x1f) as u8,
            (((raw >> 5) & 0x1f) * 0xff / 0x1f) as u8,
            (((raw >> 10) & 0x1f) * 0xff / 0x1f) as u8,
            0xff,
        ])
    }
}

pub type Palette = [Bgr555; 16];

type PalettedImage = image::ImageBuffer<image::Luma<u8>, Vec<u8>>;

pub const TILE_WIDTH: usize = 8;
pub const TILE_HEIGHT: usize = 8;
pub const TILE_BYTES: usize = TILE_WIDTH * TILE_HEIGHT / 2;

pub fn read_tile(raw: &[u8]) -> Result<PalettedImage, std::io::Error> {
    image::ImageBuffer::from_vec(
        TILE_WIDTH as u32,
        TILE_HEIGHT as u32,
        raw.iter().flat_map(|v| vec![v & 0xf, v >> 4]).collect(),
    )
    .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "buffer too small"))
}

pub fn merge_tiles(tiles: &[PalettedImage], cols: usize) -> PalettedImage {
    let rows = tiles.len() / cols;
    let mut img = image::ImageBuffer::new((cols * TILE_WIDTH) as u32, (rows * TILE_HEIGHT) as u32);
    for (i, tile) in tiles.iter().enumerate() {
        let x = i % cols;
        let y = i / cols;
        image::imageops::replace(&mut img, tile, (x * TILE_WIDTH) as i64, (y * TILE_HEIGHT) as i64);
    }
    img
}

pub fn apply_palette(paletted: PalettedImage, palette: &Palette) -> image::RgbaImage {
    image::ImageBuffer::from_vec(
        paletted.width(),
        paletted.height(),
        paletted
            .iter()
            .flat_map(|v| {
                if *v > 0 {
                    palette[*v as usize].to_rgba8()
                } else {
                    image::Rgba([0, 0, 0, 0])
                }
                .0
            })
            .collect(),
    )
    .unwrap()
}

pub fn read_merged_tiles(raw: &[u8], cols: usize) -> Result<PalettedImage, std::io::Error> {
    Ok(merge_tiles(
        &raw.chunks(TILE_BYTES).map(read_tile).collect::<Result<Vec<_>, _>>()?,
        cols,
    ))
}

pub fn unlz77(r: &mut impl std::io::Read) -> std::io::Result<Vec<u8>> {
    let mut out = vec![];

    let header = r.read_u32::<byteorder::LittleEndian>()?;
    if (header & 0xff) != 0x10 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid header"));
    }

    let n = (header >> 8) as usize;
    while out.len() < n {
        let ref_ = r.read_u8()?;

        for i in 0..8 {
            if out.len() >= n {
                break;
            }

            if (ref_ & (0x80 >> i)) == 0 {
                out.push(r.read_u8()?);
                continue;
            }

            // Yes that's right, it's big endian here!
            let info = r.read_u16::<byteorder::BigEndian>()?;

            let m = info >> 12;
            let offset = info & 0x0fff;

            for _ in 0..(m + 3) {
                out.push(out[out.len() - offset as usize - 1]);
            }
        }
    }

    out.truncate(n);
    Ok(out)
}

pub struct MemoryMapper {
    rom: Vec<u8>,
    wram: Vec<u8>,
    unlz77_cache: std::sync::Mutex<std::collections::HashMap<u32, Vec<u8>>>,
}

impl MemoryMapper {
    pub fn new(rom: Vec<u8>, wram: Vec<u8>) -> Self {
        Self {
            rom,
            wram,
            unlz77_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub fn get(&self, start: u32) -> std::borrow::Cow<'_, [u8]> {
        #[allow(clippy::manual_range_contains)]
        if start >= 0x02000000 && start < 0x04000000 {
            std::borrow::Cow::Borrowed(&self.wram[(start & !0x02000000) as usize..])
        } else if start >= 0x08000000 && start < 0x0a000000 {
            std::borrow::Cow::Borrowed(&self.rom[(start & !0x08000000) as usize..])
        } else if start >= 0x88000000 && start < 0x8a000000 {
            std::borrow::Cow::Owned(
                self.unlz77_cache
                    .lock()
                    .unwrap()
                    .entry(start)
                    .or_insert_with(|| unlz77(&mut &self.rom[(start & !0x88000000) as usize..]).unwrap()[4..].to_vec())
                    .clone(),
            )
        } else {
            panic!("could not get slice")
        }
    }
}
