//! The underlying OsString/OsStr implementation on Unix and many other
//! systems: just a `Vec<u8>`/`[u8]`.

use crate::os_str::{OsStr, OsString};
use alloc::borrow::Cow;
use alloc::rc::Rc;
use alloc::sync::Arc;
use core::fmt;
use core::mem;
use core::str;
use core::str::from_utf8_unchecked;
// use crate::sys_common::bytestring::debug_fmt_bytestring;
// use crate::sys_common::{AsInner, FromInner, IntoInner};
use crate::alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use core::fmt::{Formatter, Write};
use core::str::Utf8Chunks;
#[doc(hidden)]
pub trait AsInner<Inner: ?Sized> {
    fn as_inner(&self) -> &Inner;
}

/// A trait for viewing representations from std types
#[doc(hidden)]
pub trait AsInnerMut<Inner: ?Sized> {
    fn as_inner_mut(&mut self) -> &mut Inner;
}

/// A trait for extracting representations from std types
#[doc(hidden)]
pub trait IntoInner<Inner> {
    fn into_inner(self) -> Inner;
}

/// A trait for creating std types from internal representations
#[doc(hidden)]
pub trait FromInner<Inner> {
    fn from_inner(inner: Inner) -> Self;
}

pub fn debug_fmt_bytestring(slice: &[u8], f: &mut Formatter<'_>) -> core::fmt::Result {
    // Writes out a valid unicode string with the correct escape sequences
    fn write_str_escaped(f: &mut Formatter<'_>, s: &str) -> core::fmt::Result {
        for c in s.chars().flat_map(|c| c.escape_debug()) {
            f.write_char(c)?
        }
        Ok(())
    }

    f.write_str("\"")?;
    // for Utf8LossyChunk { valid, broken } in Utf8Lossy::from_bytes(slice).chunks() {
    for chunk in Utf8Chunks::new(slice) {
        write_str_escaped(f, chunk.valid())?;
        for b in chunk.invalid() {
            write!(f, "\\x{:02X}", b)?;
        }
    }
    f.write_str("\"")
}

#[derive(Clone, Hash, Serialize, Deserialize)]
pub(crate) struct Buf {
    pub inner: Vec<u8>,
}

// FIXME:
// `Buf::as_slice` current implementation relies
// on `Slice` being layout-compatible with `[u8]`.
// When attribute privacy is implemented, `Slice` should be annotated as `#[repr(transparent)]`.
// Anyway, `Slice` representation and layout are considered implementation detail, are
// not documented and must not be relied upon.
#[derive(Serialize)]
pub(crate) struct Slice {
    pub inner: [u8],
}

impl fmt::Debug for Slice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_fmt_bytestring(&self.inner, formatter)
    }
}

impl fmt::Display for Slice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe { fmt::Display::fmt(from_utf8_unchecked(&self.inner), formatter) }
    }
}

impl fmt::Debug for Buf {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_slice(), formatter)
    }
}

impl fmt::Display for Buf {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_slice(), formatter)
    }
}

impl IntoInner<Vec<u8>> for Buf {
    fn into_inner(self) -> Vec<u8> {
        self.inner
    }
}

impl AsInner<[u8]> for Buf {
    fn as_inner(&self) -> &[u8] {
        &self.inner
    }
}

impl Buf {
    pub fn from_string(s: String) -> Buf {
        Buf {
            inner: s.into_bytes(),
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Buf {
        Buf {
            inner: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional)
    }

    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.inner.reserve_exact(additional)
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit()
    }

    #[inline]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.inner.shrink_to(min_capacity)
    }

    #[inline]
    pub fn as_slice(&self) -> &Slice {
        // Safety: Slice just wraps [u8],
        // and &*self.inner is &[u8], therefore
        // transmuting &[u8] to &Slice is safe.
        unsafe { mem::transmute(&*self.inner) }
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut Slice {
        // Safety: Slice just wraps [u8],
        // and &mut *self.inner is &mut [u8], therefore
        // transmuting &mut [u8] to &mut Slice is safe.
        unsafe { mem::transmute(&mut *self.inner) }
    }

    pub fn into_string(self) -> Result<String, Buf> {
        String::from_utf8(self.inner).map_err(|p| Buf {
            inner: p.into_bytes(),
        })
    }

    pub fn push_slice(&mut self, s: &Slice) {
        self.inner.extend_from_slice(&s.inner)
    }

    #[inline]
    pub fn into_box(self) -> Box<Slice> {
        unsafe { mem::transmute(self.inner.into_boxed_slice()) }
    }

    #[inline]
    pub fn from_box(boxed: Box<Slice>) -> Buf {
        let inner: Box<[u8]> = unsafe { mem::transmute(boxed) };
        Buf {
            inner: inner.into_vec(),
        }
    }

    #[inline]
    pub fn into_arc(&self) -> Arc<Slice> {
        self.as_slice().into_arc()
    }

    #[inline]
    pub fn into_rc(&self) -> Rc<Slice> {
        self.as_slice().into_rc()
    }
}

impl Slice {
    #[inline]
    fn from_u8_slice(s: &[u8]) -> &Slice {
        unsafe { mem::transmute(s) }
    }

    #[inline]
    pub fn from_str(s: &str) -> &Slice {
        Slice::from_u8_slice(s.as_bytes())
    }

    pub fn to_str(&self) -> Option<&str> {
        str::from_utf8(&self.inner).ok()
    }

    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.inner)
    }

    pub fn to_owned(&self) -> Buf {
        Buf {
            inner: self.inner.to_vec(),
        }
    }

    pub fn clone_into(&self, buf: &mut Buf) {
        self.inner.clone_into(&mut buf.inner)
    }

    #[inline]
    pub fn into_box(&self) -> Box<Slice> {
        let boxed: Box<[u8]> = self.inner.into();
        unsafe { mem::transmute(boxed) }
    }

    pub fn empty_box() -> Box<Slice> {
        let boxed: Box<[u8]> = Default::default();
        unsafe { mem::transmute(boxed) }
    }

    #[inline]
    pub fn into_arc(&self) -> Arc<Slice> {
        let arc: Arc<[u8]> = Arc::from(&self.inner);
        unsafe { Arc::from_raw(Arc::into_raw(arc) as *const Slice) }
    }

    #[inline]
    pub fn into_rc(&self) -> Rc<Slice> {
        let rc: Rc<[u8]> = Rc::from(&self.inner);
        unsafe { Rc::from_raw(Rc::into_raw(rc) as *const Slice) }
    }

    #[inline]
    pub fn make_ascii_lowercase(&mut self) {
        self.inner.make_ascii_lowercase()
    }

    #[inline]
    pub fn make_ascii_uppercase(&mut self) {
        self.inner.make_ascii_uppercase()
    }

    #[inline]
    pub fn to_ascii_lowercase(&self) -> Buf {
        Buf {
            inner: self.inner.to_ascii_lowercase(),
        }
    }

    #[inline]
    pub fn to_ascii_uppercase(&self) -> Buf {
        Buf {
            inner: self.inner.to_ascii_uppercase(),
        }
    }

    #[inline]
    pub fn is_ascii(&self) -> bool {
        self.inner.is_ascii()
    }

    #[inline]
    pub fn eq_ignore_ascii_case(&self, other: &Self) -> bool {
        self.inner.eq_ignore_ascii_case(&other.inner)
    }
}

/// Platform-specific extensions to [`OsString`].
///
/// [`OsString`]: ../../../../std/ffi/struct.OsString.html
//#[stable(feature = "rust1", since = "1.0.0")]
pub trait OsStringExt {
    /// Creates an [`OsString`] from a byte vector.
    ///
    /// See the module documentation for an example.
    ///
    /// [`OsString`]: ../../../ffi/struct.OsString.html
    //#[stable(feature = "rust1", since = "1.0.0")]
    fn from_vec(vec: Vec<u8>) -> Self;

    /// Yields the underlying byte vector of this [`OsString`].
    ///
    /// See the module documentation for an example.
    ///
    /// [`OsString`]: ../../../ffi/struct.OsString.html
    //#[stable(feature = "rust1", since = "1.0.0")]
    fn into_vec(self) -> Vec<u8>;
}

//#[stable(feature = "rust1", since = "1.0.0")]
impl OsStringExt for OsString {
    fn from_vec(vec: Vec<u8>) -> OsString {
        FromInner::from_inner(Buf { inner: vec })
    }
    fn into_vec(self) -> Vec<u8> {
        self.into_inner().inner
    }
}

/// Platform-specific extensions to [`OsStr`].
///
/// [`OsStr`]: ../../../../std/ffi/struct.OsStr.html
//#[stable(feature = "rust1", since = "1.0.0")]
pub trait OsStrExt {
    //#[stable(feature = "rust1", since = "1.0.0")]
    /// Creates an [`OsStr`] from a byte slice.
    ///
    /// See the module documentation for an example.
    ///
    /// [`OsStr`]: ../../../ffi/struct.OsStr.html
    fn from_bytes(slice: &[u8]) -> &Self;

    /// Gets the underlying byte view of the [`OsStr`] slice.
    ///
    /// See the module documentation for an example.
    ///
    /// [`OsStr`]: ../../../ffi/struct.OsStr.html
    //#[stable(feature = "rust1", since = "1.0.0")]
    fn as_bytes(&self) -> &[u8];
}

//#[stable(feature = "rust1", since = "1.0.0")]
impl OsStrExt for OsStr {
    #[inline]
    fn from_bytes(slice: &[u8]) -> &OsStr {
        unsafe { mem::transmute(slice) }
    }
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        &self.as_inner().inner
    }
}
