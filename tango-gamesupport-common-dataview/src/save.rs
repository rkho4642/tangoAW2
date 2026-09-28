pub trait SaveClone {
    fn clone_box(&self) -> Box<dyn Save + Sync + Send>;
}

impl<T> SaveClone for T
where
    T: 'static + Save + Sync + Send + Clone,
{
    fn clone_box(&self) -> Box<dyn Save + Sync + Send> {
        Box::new(self.clone())
    }
}

/// `Any`-upcast, blanket-implemented for every concrete type so trait
/// objects ([`Save`], [`crate::rom::Assets`]) can be downcast to their
/// game's concrete type. This is how game-specific model surface stays
/// out of the shared traits: the game's own code downcasts and uses the
/// concrete API.
///
/// Careful with boxes: the blanket impl covers `Box<dyn Save>` itself
/// too, so `boxed.as_any()` answers for the *box*. Go through the trait
/// object — `boxed.as_ref().as_any()` / `boxed.as_mut().as_any_mut()` —
/// to reach the concrete save.
pub trait AsAny {
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T: std::any::Any> AsAny for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// A parsed save, as a game crate implements it: enough to write it back
/// out, hand its WRAM image to the ROM asset loader, and repair its
/// checksum. Anything a game models beyond that stays on its concrete
/// type (reach it via [`AsAny`]).
pub trait Save
where
    Self: SaveClone + AsAny,
{
    fn to_sram_dump(&self) -> Vec<u8>;
    /// The save's WRAM image, as [`crate::model::prepare`] passes it to
    /// the game's ROM asset loader.
    fn as_raw_wram(&self) -> std::borrow::Cow<'_, [u8]>;

    fn rebuild_checksum(&mut self);

    /// Return typed findings only when there are violations.
    fn game_violations(&self, _assets: &dyn crate::rom::Assets) -> Option<Box<dyn std::any::Any + Send + Sync>> {
        None
    }
}

impl Clone for Box<dyn Save + Send + Sync> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub fn mask(buf: &mut [u8], mask_offset: usize) {
    let mask = bytemuck::pod_read_unaligned::<u32>(&buf[mask_offset..][..std::mem::size_of::<u32>()]);
    for b in buf.iter_mut() {
        *b ^= mask as u8;
    }
    buf[mask_offset..][..std::mem::size_of::<u32>()].copy_from_slice(bytemuck::bytes_of(&mask));
}

pub fn compute_raw_checksum(buf: &[u8], checksum_offset: usize) -> u32 {
    buf.iter().map(|v| *v as u32).sum::<u32>()
        - buf[checksum_offset..][..std::mem::size_of::<u32>()]
            .iter()
            .map(|v| *v as u32)
            .sum::<u32>()
}

/// The size of the GBA SRAM dump a save is written back out as.
pub const SRAM_DUMP_SIZE: usize = 0x10000;

/// The `N`-byte save image starting at `start` in `buf`, or
/// [`Error::InvalidSize`] (with `buf`'s whole length) when `buf` stops
/// short of it.
pub fn read_image<const N: usize>(buf: &[u8], start: usize) -> Result<[u8; N], Error> {
    buf.get(start..)
        .and_then(|buf| buf.get(..N))
        .and_then(|buf| buf.try_into().ok())
        .ok_or(Error::InvalidSize(buf.len()))
}

/// The u32 checksum stored at `checksum_offset`.
pub fn read_checksum_u32(buf: &[u8], checksum_offset: usize) -> u32 {
    bytemuck::pod_read_unaligned::<u32>(&buf[checksum_offset..][..std::mem::size_of::<u32>()])
}

/// Store `checksum` as the u32 at `checksum_offset`.
pub fn write_checksum_u32(buf: &mut [u8], checksum_offset: usize, checksum: u32) {
    buf[checksum_offset..][..std::mem::size_of::<u32>()].copy_from_slice(bytemuck::bytes_of(&checksum));
}

/// `Ok` when the stored checksum `actual` is the `computed` one, else the
/// single-candidate [`Error::ChecksumMismatch`].
pub fn verify_checksum(actual: u32, computed: u32, shift: usize) -> Result<(), Error> {
    if actual != computed {
        return Err(Error::ChecksumMismatch {
            actual,
            expected: vec![computed],
            shift,
        });
    }
    Ok(())
}

/// A [`SRAM_DUMP_SIZE`] SRAM dump holding `image` at `start`, masked in
/// place with the mask word at `mask_offset` for games that mask their
/// save.
pub fn sram_dump(image: &[u8], start: usize, mask_offset: Option<usize>) -> Vec<u8> {
    let mut buf = vec![0; SRAM_DUMP_SIZE];
    let dst = &mut buf[start..][..image.len()];
    dst.copy_from_slice(image);
    if let Some(mask_offset) = mask_offset {
        mask(dst, mask_offset);
    }
    buf
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid size: {0} bytes")]
    InvalidSize(usize),

    #[error("invalid game name: {0:02x?}")]
    InvalidGameName(Vec<u8>),

    #[error("invalid checksum: {actual:08x} not in {expected:08x?} (shift: {shift})")]
    ChecksumMismatch {
        expected: Vec<u32>,
        actual: u32,
        shift: usize,
    },

    #[error("invalid shift: {0}")]
    InvalidShift(usize),
}
