//! `farm-ffi`: the C ABI that `FarmEngine.Interop` (C#) binds to.
//!
//! Conventions (docs/dev/ARCHITECTURE.md "FFI: .NET → Rust"):
//! - coarse, handle-based, batched calls; never one call per tile or entity;
//! - Rust allocates result buffers, .NET copies what it needs and frees them with
//!   [`fe_bytes_free`]; no pointer into Rust memory outlives the next call on that handle;
//! - every call on a handle (session, preview, player) reports failures the same way: the result
//!   code says what happened, `out` holds the UTF-8 error message (for
//!   [`FeResult::Poisoned`], the error that poisoned the handle), and `fe_<handle>_last_error`
//!   returns the last message again;
//! - panics are caught at the boundary and returned as error results, never unwound into .NET,
//!   and that includes freeing a handle;
//! - [`fe_abi_version`] changes whenever a signature or a buffer layout does: .NET refuses a
//!   library whose version it was not built for.
//!
//! `lib.rs` has the primitives (versions, stable hash, buffers); [`session`] has the engine
//! sessions (create from project JSON, apply commands, tick, read state/hash/views); [`render`]
//! has `farm-render` requests and the Edit Mode map preview; [`player`] the graphical player;
//! [`audio`] decodes and previews sound and music assets.
//!
//! The requests and answers themselves live in `farm-host`, which farm-wasm shares: this crate
//! only moves bytes across the C ABI and maps [`farm_host::HostError`]s to [`FeResult`]s.

use std::panic::{catch_unwind, AssertUnwindSafe};

pub mod audio;
pub mod player;
pub mod render;
pub mod session;
/// JSON for host views in engine order (moved to `farm-host`, shared with farm-wasm).
pub use farm_host::view_json;
pub use render::FePreview;
pub use session::FeSession;

/// The version of this C ABI: bumped on every change to an export's signature, a buffer's
/// layout or the error convention. `FarmEngine.Interop` compares it when it loads the library.
///
/// - 2: errors in `out` for every handle, `fe_*_last_error` for previews and players,
///   `fe_player_frame_info` / `fe_player_copy_pixels`, `fe_preview_set_scene`.
/// - 3: `fe_hash_text` is gone.
/// - 4: `fe_audio_output`, `fe_audio_probe` and `fe_audio_preview_{play,stop,playing}`.
pub const FE_ABI_VERSION: u32 = 4;

/// A Rust-allocated byte buffer handed to .NET. Free it with [`fe_bytes_free`].
#[repr(C)]
#[derive(Debug)]
pub struct FeBytes {
    pub ptr: *mut u8,
    pub len: usize,
    pub cap: usize,
}

impl FeBytes {
    pub(crate) fn from_vec(mut vec: Vec<u8>) -> Self {
        let bytes = Self { ptr: vec.as_mut_ptr(), len: vec.len(), cap: vec.capacity() };
        std::mem::forget(vec);
        bytes
    }

    pub(crate) fn empty() -> Self {
        Self { ptr: std::ptr::null_mut(), len: 0, cap: 0 }
    }
}

/// Result codes shared by every call.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeResult {
    Ok = 0,
    InvalidArgument = 1,
    Panic = 2,
    /// A previous call on this handle panicked (or stopped the game); its state is not
    /// trustworthy.
    Poisoned = 3,
}

impl From<farm_host::ErrorKind> for FeResult {
    fn from(kind: farm_host::ErrorKind) -> Self {
        match kind {
            farm_host::ErrorKind::Invalid => FeResult::InvalidArgument,
            farm_host::ErrorKind::Panic => FeResult::Panic,
            farm_host::ErrorKind::Poisoned => FeResult::Poisoned,
        }
    }
}

/// The `len` bytes at `ptr`; null is accepted only with length 0.
///
/// # Safety
/// A non-null `ptr` must point to `len` readable bytes that stay valid and unchanged for `'a`.
pub(crate) unsafe fn bytes_arg<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if ptr.is_null() {
        (len == 0).then_some(&[])
    } else {
        // SAFETY: the caller guarantees `len` readable bytes at the non-null `ptr`.
        Some(unsafe { std::slice::from_raw_parts(ptr, len) })
    }
}

/// Hands `bytes` to the caller through `out` (ignored when `out` is null). Whatever `out`
/// held is overwritten without being read or dropped.
///
/// # Safety
/// A non-null `out` must be valid for writes.
pub(crate) unsafe fn write(out: *mut FeBytes, bytes: Vec<u8>) {
    if !out.is_null() {
        // SAFETY: the caller guarantees `out` is writable; `write` never reads the old value.
        unsafe { out.write(FeBytes::from_vec(bytes)) };
    }
}

/// Sets `out` to an empty buffer (ignored when `out` is null).
///
/// # Safety
/// A non-null `out` must be valid for writes.
pub(crate) unsafe fn write_empty(out: *mut FeBytes) {
    if !out.is_null() {
        // SAFETY: as in `write`.
        unsafe { out.write(FeBytes::empty()) };
    }
}

/// Writes a failed call's message to `out` and turns its kind into the result code: the one
/// error convention of every handle.
///
/// # Safety
/// A non-null `out` must be valid for writes.
pub(crate) unsafe fn fail(out: *mut FeBytes, error: farm_host::HostError) -> FeResult {
    // SAFETY: forwarded from the caller.
    unsafe { write(out, error.message.into_bytes()) };
    error.kind.into()
}

/// Frees a handle's contents without letting a panic in their drop code (a plugin runtime, an
/// audio thread being joined) unwind into .NET, which would abort the process. With `leak` the
/// value is not dropped at all: a handle poisoned by a panic may hold state its drop code cannot
/// cope with, and leaking it is safer than running that code.
pub(crate) fn release<T>(value: T, leak: bool) {
    if leak {
        std::mem::forget(value);
    } else {
        // A panicking drop has already dropped what it could; nothing else to do.
        let _ = catch_unwind(AssertUnwindSafe(move || drop(value)));
    }
}

/// The crate version as a NUL-terminated UTF-8 string (static; do not free).
#[no_mangle]
pub extern "C" fn fe_version() -> *const std::os::raw::c_char {
    static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr().cast()
}

/// The version of this C ABI ([`FE_ABI_VERSION`]). Callers compare it before any other call:
/// a library built from other sources may have different signatures.
#[no_mangle]
pub extern "C" fn fe_abi_version() -> u32 {
    FE_ABI_VERSION
}

/// Frees a buffer returned by this library. Passing an empty buffer is a no-op.
///
/// # Safety
/// `bytes` must have come from this library and must not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn fe_bytes_free(bytes: FeBytes) {
    if !bytes.ptr.is_null() {
        // SAFETY: the buffer came from `FeBytes::from_vec`, so these are a `Vec<u8>`'s parts,
        // and the caller hands it back exactly once.
        drop(unsafe { Vec::from_raw_parts(bytes.ptr, bytes.len, bytes.cap) });
    }
}

#[cfg(test)]
#[allow(clippy::undocumented_unsafe_blocks)]
mod tests {
    use super::*;

    #[test]
    fn the_abi_version_is_exported() {
        assert_eq!(fe_abi_version(), FE_ABI_VERSION);
    }

    #[test]
    fn release_survives_a_panicking_drop() {
        struct Bomb;
        impl Drop for Bomb {
            fn drop(&mut self) {
                panic!("drop failed");
            }
        }
        release(Bomb, false);
        release(Bomb, true);
    }
}
