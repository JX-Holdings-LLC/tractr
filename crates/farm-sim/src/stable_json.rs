//! Port of `stableStringify` (engine-core/src/hash.ts, `FarmEngine.Json.StableJson`): JSON with
//! object keys sorted by UTF-16 code unit and JavaScript number/string formatting, so a Rust
//! state and a TypeScript state that are equal produce identical text.
//!
//! [`stringify`] writes the text straight from `Serialize`, without building a
//! `serde_json::Value` tree first: each object is written in the order its fields arrive, and
//! only an object whose keys did not arrive in stable order is rearranged, by copying its
//! members once into sorted order. Saving the state runs this on the simulation thread
//! (docs/LANGUAGES.md "Saves"). The first version built the tree and sorted it
//! ([`stringify_via_value`]); it stays as the reference the writer is tested against, and as
//! the fallback for what the writer does not handle itself (a map key that is not a string or
//! an integer, a 128-bit integer beyond 64 bits, an `f32`), so the output never depends on
//! which of the two wrote it.

use crate::{text, units};
use serde::ser::{self, Serialize};
use serde_json::Value;
use std::cmp::Ordering;

/// Stable JSON text of any serializable value.
pub fn stringify<T: Serialize + ?Sized>(value: &T) -> String {
    try_stringify(value).expect("engine types always serialize")
}

/// Stable JSON text of a JSON value.
pub fn stringify_value(value: &Value) -> String {
    stringify(value)
}

/// Stable JSON text of any serializable value, or the serializer's error (where
/// `serde_json::to_value` fails).
pub fn try_stringify<T: Serialize + ?Sized>(value: &T) -> Result<String, serde_json::Error> {
    let mut writer = Writer::default();
    match value.serialize(&mut writer).and_then(|()| String::from_utf8(writer.out).map_err(|_| Unsupported)) {
        Ok(text) => Ok(text),
        // Leave anything unusual to the reference path, which then decides.
        Err(_) => try_stringify_via_value(value),
    }
}

/// The reference implementation: `serde_json::to_value`, then the value tree written with its
/// object keys sorted. Same text as [`stringify`], about five times slower.
pub fn stringify_via_value<T: Serialize + ?Sized>(value: &T) -> String {
    try_stringify_via_value(value).expect("engine types always serialize")
}

fn try_stringify_via_value<T: Serialize + ?Sized>(value: &T) -> Result<String, serde_json::Error> {
    let json = serde_json::to_value(value)?;
    Ok(stringify_value_tree(&json))
}

/// [`stringify_value`] by the reference implementation (walking the tree).
pub fn stringify_value_tree(value: &Value) -> String {
    let mut out = String::new();
    write(&mut out, value);
    out
}

/// True for canonical array-index strings: "0" or no leading zero, value ≤ 2^32 − 2.
pub fn array_index(key: &str) -> Option<u32> {
    if key.is_empty() || key.len() > 10 {
        return None;
    }
    if key.len() > 1 && key.starts_with('0') {
        return None;
    }
    let mut value: u64 = 0;
    for c in key.bytes() {
        if !c.is_ascii_digit() {
            return None;
        }
        value = value * 10 + u64::from(c - b'0');
    }
    if value > 4_294_967_294 {
        return None;
    }
    Some(value as u32)
}

/// JS objects enumerate array-index keys ("0".."4294967294", canonical form) first in ascending
/// numeric order, then the remaining keys in insertion order. `stableStringify` inserts keys
/// sorted, so the final order is: index keys numerically, then the rest as sorted.
pub fn order_like_js_object<T>(items: Vec<T>, key: impl Fn(&T) -> &str) -> Vec<T> {
    let mut index_keys: Vec<(u32, T)> = Vec::new();
    let mut rest: Vec<T> = Vec::new();
    for item in items {
        match array_index(key(&item)) {
            Some(index) => index_keys.push((index, item)),
            None => rest.push(item),
        }
    }
    if index_keys.is_empty() {
        return rest;
    }
    index_keys.sort_by_key(|(index, _)| *index);
    index_keys.into_iter().map(|(_, item)| item).chain(rest).collect()
}

/// The order of object keys in stable JSON, as one comparison: array-index keys first by
/// value, then the other keys by UTF-16 code units (what sorting by [`text::compare_strings`]
/// and then [`order_like_js_object`] gives).
pub fn compare_keys(a: &str, b: &str) -> Ordering {
    let a_digit = a.as_bytes().first().is_some_and(u8::is_ascii_digit);
    let b_digit = b.as_bytes().first().is_some_and(u8::is_ascii_digit);
    if a_digit || b_digit {
        match (array_index(a), array_index(b)) {
            (Some(x), Some(y)) => return x.cmp(&y),
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            (None, None) => {}
        }
    }
    if a.is_ascii() && b.is_ascii() {
        // ASCII is one UTF-16 code unit per byte.
        return a.as_bytes().cmp(b.as_bytes());
    }
    text::compare_strings(a, b)
}

fn write(out: &mut String, value: &Value) {
    match value {
        Value::Object(map) => {
            // Later duplicates win (JS object semantics); serde_json objects have no duplicates.
            let mut props: Vec<(&str, &Value)> = map.iter().map(|(k, v)| (k.as_str(), v)).collect();
            props.sort_by(|a, b| text::compare_strings(a.0, b.0));
            let props = order_like_js_object(props, |p| p.0);
            out.push('{');
            for (i, (name, item)) in props.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                text::push_quoted(out, name);
                out.push(':');
                write(out, item);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(out, item);
            }
            out.push(']');
        }
        Value::String(s) => text::push_quoted(out, s),
        Value::Number(n) => {
            // Integers keep every digit (a double would round those above 2⁵³, #142); doubles
            // print like JS. JSON.stringify writes non-finite numbers as null.
            if let Some(int) = n.as_i64() {
                out.push_str(&int.to_string());
            } else if let Some(int) = n.as_u64() {
                out.push_str(&int.to_string());
            } else {
                match n.as_f64() {
                    Some(d) if d.is_finite() => out.push_str(&units::format_number(d)),
                    _ => out.push_str("null"),
                }
            }
        }
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Null => out.push_str("null"),
    }
}

// --- The direct writer -------------------------------------------------------------------------

/// Why the direct writer gave up: something it leaves to the reference path.
#[derive(Debug)]
struct Unsupported;

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("left to the reference stable JSON writer")
    }
}

impl std::error::Error for Unsupported {}

impl ser::Error for Unsupported {
    fn custom<T: std::fmt::Display>(_msg: T) -> Self {
        Unsupported
    }
}

type Written<T = ()> = std::result::Result<T, Unsupported>;

/// Marks a [`KeyRef::len`] whose text is in [`Writer::keys`] rather than in the output.
const KEPT: u32 = 1 << 31;

/// Where a key's text is: between its quotes in [`Writer::out`], or, when escaping changed it,
/// in [`Writer::keys`] (`len` has [`KEPT`] set). With the key's [`key_rank`].
#[derive(Clone, Copy)]
struct KeyRef {
    rank: u128,
    start: u32,
    len: u32,
}

/// One member of an open object: its key, and where `"key":value` is in the output.
#[derive(Clone, Copy)]
struct Member {
    key: KeyRef,
    start: u32,
    end: u32,
}

impl Member {
    fn range(&self) -> std::ops::Range<usize> {
        self.start as usize..self.end as usize
    }
}

fn position(at: usize) -> Written<u32> {
    // Past 4 GiB the reference path writes it.
    u32::try_from(at).map_err(|_| Unsupported)
}

fn key_len(len: usize) -> Written<u32> {
    position(len).and_then(|len| if len & KEPT == 0 { Ok(len) } else { Err(Unsupported) })
}

/// A number that orders keys like [`compare_keys`] wherever two ranks differ, so most
/// comparisons are one integer comparison. Array-index keys rank by their value, below every
/// other key; another key ranks by its first seven UTF-16 code units (zero-padded), then by its
/// length in code units up to 8. Equal ranks need [`compare_keys`], unless the rank
/// [is exact](rank_is_exact).
fn key_rank(key: &[u8]) -> u128 {
    if key.first().is_some_and(u8::is_ascii_digit) {
        if let Some(index) = array_index_bytes(key) {
            return u128::from(index);
        }
    }
    let head = match key.first_chunk::<8>() {
        Some(head) => u64::from_be_bytes(*head),
        None => key.iter().enumerate().fold(0, |word, (i, &byte)| word | u64::from(byte) << (56 - 8 * i)),
    };
    let (prefix, units) = if head & 0x8080_8080_8080_8000 == 0 {
        // The first seven bytes are ASCII, one code unit each: spread them into 16-bit lanes.
        let mut x = u128::from(head >> 8);
        x = (x | x << 32) & 0x0000_0000_FFFF_FFFF_0000_0000_FFFF_FFFF;
        x = (x | x << 16) & 0x0000_FFFF_0000_FFFF_0000_FFFF_0000_FFFF;
        x = (x | x << 8) & 0x00FF_00FF_00FF_00FF_00FF_00FF_00FF_00FF;
        (x, key.len().min(8))
    } else {
        let text = std::str::from_utf8(key).unwrap_or_default();
        let mut prefix = 0;
        let mut units = 0;
        for unit in text.encode_utf16().take(8) {
            if units < 7 {
                prefix = prefix << 16 | u128::from(unit);
            }
            units += 1;
        }
        (prefix << (16 * (7 - units.min(7))), units)
    };
    1 << 127 | prefix << 4 | units as u128
}

/// Does `rank` tell its key's whole text (an array index, or at most seven code units)?
fn rank_is_exact(rank: u128) -> bool {
    rank >> 127 == 0 || rank & 0xf < 8
}

/// [`array_index`] of a key's bytes.
fn array_index_bytes(key: &[u8]) -> Option<u32> {
    std::str::from_utf8(key).ok().and_then(array_index)
}

/// [`compare_keys`] of two keys' bytes (UTF-8 written from `&str`s).
fn compare_key_bytes(a: &[u8], b: &[u8]) -> Ordering {
    match (std::str::from_utf8(a), std::str::from_utf8(b)) {
        (Ok(a), Ok(b)) => compare_keys(a, b),
        _ => a.cmp(b),
    }
}

/// Writes stable JSON from `Serialize` (see the module docs). The members and escaped keys of
/// the open objects live on stacks, so a nested object needs no allocation of its own.
#[derive(Default)]
struct Writer {
    out: Vec<u8>,
    /// The members of every open object, innermost last.
    members: Vec<Member>,
    /// The text of the escaped keys of every open object.
    keys: Vec<u8>,
    /// Where an object's members are copied to while they are put in order.
    scratch: Vec<u8>,
    /// The orders worked out for objects whose keys arrived out of order, by the hash of their
    /// key ranks: the objects of one type mostly arrive with the same keys in the same order,
    /// so their order is sorted out once.
    shapes: Vec<Option<Shape>>,
}

/// How many [`Shape`]s the writer keeps, and the most members an object may have to be kept.
const SHAPE_SLOTS: usize = 64;
const SHAPE_MAX_MEMBERS: usize = 64;

/// A member at least this long moves inside the output when its object is put in order,
/// instead of through [`Writer::scratch`] and back.
const MOVE_IN_PLACE: usize = 4096;

/// The keys of an object, as they arrived, and the order to write its members in.
struct Shape {
    signature: u64,
    ranks: Vec<u128>,
    /// The members whose rank is not [exact](rank_is_exact), with their key text.
    long_keys: Vec<(usize, Vec<u8>)>,
    /// Indices of the members in stable order, without the members a later duplicate replaces.
    order: Vec<u32>,
}

impl Writer {
    fn push_string(&mut self, value: &str) {
        push_quoted(&mut self.out, value);
    }

    /// Writes a key, quoted, and says where its text is.
    #[inline]
    fn push_key(&mut self, key: &str) -> Written<KeyRef> {
        let rank = key_rank(key.as_bytes());
        let start = self.out.len() + 1;
        if push_quoted(&mut self.out, key) {
            let start = position(self.keys.len())?;
            self.keys.extend_from_slice(key.as_bytes());
            return Ok(KeyRef { rank, start, len: key_len(key.len())? | KEPT });
        }
        Ok(KeyRef { rank, start: position(start)?, len: key_len(key.len())? })
    }

    fn push_digits(&mut self, value: u64, negative: bool) {
        push_digits(&mut self.out, value, negative);
    }

    fn begin_object(&mut self) -> Object<'_> {
        self.out.push(b'{');
        Object {
            body_start: self.out.len(),
            member_base: self.members.len(),
            key_base: self.keys.len(),
            in_order: true,
            signature: 0,
            member_start: 0,
            key: None,
            close_variant: false,
            writer: self,
        }
    }
}

#[inline]
fn key_text<'k>(out: &'k [u8], keys: &'k [u8], key: KeyRef) -> &'k [u8] {
    let len = (key.len & !KEPT) as usize;
    let start = key.start as usize;
    if key.len & KEPT == 0 {
        &out[start..start + len]
    } else {
        &keys[start..start + len]
    }
}

#[inline]
fn key_order(out: &[u8], keys: &[u8], a: KeyRef, b: KeyRef) -> Ordering {
    match a.rank.cmp(&b.rank) {
        Ordering::Equal if !rank_is_exact(a.rank) => compare_key_bytes(key_text(out, keys, a), key_text(out, keys, b)),
        order => order,
    }
}

impl Shape {
    fn matches(&self, signature: u64, out: &[u8], keys: &[u8], members: &[Member]) -> bool {
        self.signature == signature
            && self.ranks.len() == members.len()
            && self.ranks.iter().zip(members).all(|(rank, member)| *rank == member.key.rank)
            && self.long_keys.iter().all(|(i, text)| key_text(out, keys, members[*i].key) == text.as_slice())
    }
}

/// The order to write `members` in: stable order of their keys (a sort that keeps equal keys in
/// arrival order), and of equal keys only the last, which wins as in a JS object.
fn member_write_order(out: &[u8], keys: &[u8], members: &[Member]) -> Vec<u32> {
    let mut order: Vec<u32> = (0..members.len() as u32).collect();
    order.sort_by(|&a, &b| key_order(out, keys, members[a as usize].key, members[b as usize].key));
    let mut kept = Vec::with_capacity(order.len());
    for (i, &index) in order.iter().enumerate() {
        let replaced = order.get(i + 1).is_some_and(|&next| {
            key_order(out, keys, members[index as usize].key, members[next as usize].key) == Ordering::Equal
        });
        if !replaced {
            kept.push(index);
        }
    }
    kept
}

/// Rewrites the members of an object, which run from `body_start` to the end of `out`, in
/// `order`, comma-separated.
fn rearrange(out: &mut Vec<u8>, scratch: &mut Vec<u8>, body_start: usize, members: &[Member], order: &[u32]) {
    let longest = order.iter().copied().max_by_key(|&i| members[i as usize].range().len());
    match longest {
        Some(longest) if members[longest as usize].range().len() >= MOVE_IN_PLACE => {
            // Copy the others out, move the long one to its place, then put the others around it.
            scratch.clear();
            let mut at = body_start;
            let mut target = body_start;
            for (n, &i) in order.iter().enumerate() {
                let member = &members[i as usize];
                if n > 0 {
                    at += 1;
                }
                if i == longest {
                    target = at;
                } else {
                    scratch.extend_from_slice(&out[member.range()]);
                }
                at += member.range().len();
            }
            let end = at;
            out.copy_within(members[longest as usize].range(), target);
            let mut at = body_start;
            let mut from = 0;
            for (n, &i) in order.iter().enumerate() {
                if n > 0 {
                    out[at] = b',';
                    at += 1;
                }
                let len = members[i as usize].range().len();
                if i != longest {
                    out[at..at + len].copy_from_slice(&scratch[from..from + len]);
                    from += len;
                }
                at += len;
            }
            out.truncate(end);
        }
        _ => {
            scratch.clear();
            scratch.extend_from_slice(&out[body_start..]);
            out.truncate(body_start);
            for (n, &i) in order.iter().enumerate() {
                if n > 0 {
                    out.push(b',');
                }
                let range = members[i as usize].range();
                out.extend_from_slice(&scratch[range.start - body_start..range.end - body_start]);
            }
        }
    }
}

/// Decimal digits of an integer, as `to_string` writes them.
#[inline]
fn push_digits(out: &mut Vec<u8>, mut value: u64, negative: bool) {
    if value < 10 && !negative {
        out.push(b'0' + value as u8);
        return;
    }
    let mut buffer = [0u8; 21];
    let mut at = buffer.len();
    while value >= 100 {
        let pair = (value % 100) as usize * 2;
        value /= 100;
        at -= 2;
        buffer[at..at + 2].copy_from_slice(&DIGIT_PAIRS[pair..pair + 2]);
    }
    if value >= 10 {
        let pair = value as usize * 2;
        at -= 2;
        buffer[at..at + 2].copy_from_slice(&DIGIT_PAIRS[pair..pair + 2]);
    } else {
        at -= 1;
        buffer[at] = b'0' + value as u8;
    }
    if negative {
        at -= 1;
        buffer[at] = b'-';
    }
    out.extend_from_slice(&buffer[at..]);
}

/// "00" to "99".
static DIGIT_PAIRS: [u8; 200] = {
    let mut table = [0; 200];
    let mut i = 0;
    while i < 100 {
        table[2 * i] = b'0' + (i / 10) as u8;
        table[2 * i + 1] = b'0' + (i % 10) as u8;
        i += 1;
    }
    table
};

/// `JSON.stringify` quoting, like [`text::push_quoted`]: appends `value` in quotes, escaped, and
/// says whether anything needed an escape.
#[inline]
fn push_quoted(out: &mut Vec<u8>, value: &str) -> bool {
    let bytes = value.as_bytes();
    out.reserve(bytes.len() + 2);
    out.push(b'"');
    // Without an early exit, so the check runs over whole words.
    if !bytes.iter().fold(false, |escape, &byte| escape | (byte < 0x20) | (byte == b'"') | (byte == b'\\')) {
        out.extend_from_slice(bytes);
        out.push(b'"');
        return false;
    }
    let mut run = 0;
    for (i, &byte) in bytes.iter().enumerate() {
        if !ESCAPED[usize::from(byte)] {
            continue;
        }
        out.extend_from_slice(&bytes[run..i]);
        match byte {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            0x08 => out.extend_from_slice(b"\\b"),
            0x0c => out.extend_from_slice(b"\\f"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            control => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                out.extend_from_slice(b"\\u00");
                out.push(HEX[usize::from(control >> 4)]);
                out.push(HEX[usize::from(control & 0xf)]);
            }
        }
        run = i + 1;
    }
    out.extend_from_slice(&bytes[run..]);
    out.push(b'"');
    run > 0
}

/// The bytes `JSON.stringify` escapes: control characters, `"` and `\`.
static ESCAPED: [bool; 256] = {
    let mut table = [false; 256];
    let mut byte = 0;
    while byte < 0x20 {
        table[byte] = true;
        byte += 1;
    }
    table[b'"' as usize] = true;
    table[b'\\' as usize] = true;
    table
};

/// The multiplier of [`Object::signature`] (the 64-bit golden ratio).
const K: u64 = 0x9E37_79B9_7F4A_7C15;

/// An open JSON object (a struct, a map, or the inner object of a struct variant).
struct Object<'w> {
    writer: &'w mut Writer,
    /// Just after the `{`.
    body_start: usize,
    member_base: usize,
    key_base: usize,
    /// Every key so far came after the one before it in stable order (no duplicates either).
    in_order: bool,
    /// A hash of the key ranks so far, for [`Writer::shapes`].
    signature: u64,
    /// Where the pending map entry (`key` set, value not yet written) starts.
    member_start: usize,
    key: Option<KeyRef>,
    /// Also close the `{"Variant":` around this object.
    close_variant: bool,
}

impl Object<'_> {
    #[inline]
    fn start_member(&mut self) -> usize {
        if self.writer.members.len() > self.member_base {
            self.writer.out.push(b',');
        }
        self.writer.out.len()
    }

    #[inline]
    fn finish_member(&mut self, key: KeyRef, start: usize) -> Written {
        let w = &mut *self.writer;
        let member = Member { key, start: position(start)?, end: position(w.out.len())? };
        self.signature =
            (self.signature.rotate_left(7) ^ (key.rank as u64) ^ ((key.rank >> 64) as u64)).wrapping_mul(K);
        if self.in_order && w.members.len() > self.member_base {
            let previous = w.members[w.members.len() - 1].key;
            if key_order(&w.out, &w.keys, previous, key) != Ordering::Less {
                self.in_order = false;
            }
        }
        w.members.push(member);
        Ok(())
    }

    fn field<T: Serialize + ?Sized>(&mut self, name: &'static str, value: &T) -> Written {
        let start = self.start_member();
        let key = self.writer.push_key(name)?;
        self.writer.out.push(b':');
        value.serialize(&mut *self.writer)?;
        self.finish_member(key, start)
    }

    fn end(self) -> Written {
        let w = self.writer;
        if !self.in_order {
            let Writer { out, members, keys, scratch, shapes } = w;
            let members = &members[self.member_base..];
            let fresh;
            let order: &[u32] = if members.len() <= SHAPE_MAX_MEMBERS {
                if shapes.is_empty() {
                    shapes.resize_with(SHAPE_SLOTS, || None);
                }
                let slot = &mut shapes[(self.signature % SHAPE_SLOTS as u64) as usize];
                if !slot.as_ref().is_some_and(|shape| shape.matches(self.signature, out, keys, members)) {
                    *slot = Some(Shape {
                        signature: self.signature,
                        ranks: members.iter().map(|member| member.key.rank).collect(),
                        long_keys: (members.iter().enumerate())
                            .filter(|(_, member)| !rank_is_exact(member.key.rank))
                            .map(|(i, member)| (i, key_text(out, keys, member.key).to_vec()))
                            .collect(),
                        order: member_write_order(out, keys, members),
                    });
                }
                &slot.as_ref().expect("just filled").order
            } else {
                fresh = member_write_order(out, keys, members);
                &fresh
            };
            rearrange(out, scratch, self.body_start, members, order);
        }
        w.members.truncate(self.member_base);
        w.keys.truncate(self.key_base);
        w.out.push(b'}');
        if self.close_variant {
            w.out.push(b'}');
        }
        Ok(())
    }
}

impl ser::SerializeStruct for Object<'_> {
    type Ok = ();
    type Error = Unsupported;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, name: &'static str, value: &T) -> Written {
        self.field(name, value)
    }

    fn end(self) -> Written {
        Object::end(self)
    }
}

impl ser::SerializeStructVariant for Object<'_> {
    type Ok = ();
    type Error = Unsupported;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, name: &'static str, value: &T) -> Written {
        self.field(name, value)
    }

    fn end(self) -> Written {
        Object::end(self)
    }
}

impl ser::SerializeMap for Object<'_> {
    type Ok = ();
    type Error = Unsupported;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Written {
        self.member_start = self.start_member();
        let key = key.serialize(KeyWriter { writer: &mut *self.writer })?;
        self.writer.out.push(b':');
        self.key = Some(key);
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Written {
        let key = self.key.take().ok_or(Unsupported)?;
        value.serialize(&mut *self.writer)?;
        self.finish_member(key, self.member_start)
    }

    fn serialize_entry<K: Serialize + ?Sized, V: Serialize + ?Sized>(&mut self, key: &K, value: &V) -> Written {
        let start = self.start_member();
        let key = key.serialize(KeyWriter { writer: &mut *self.writer })?;
        self.writer.out.push(b':');
        value.serialize(&mut *self.writer)?;
        self.finish_member(key, start)
    }

    fn end(self) -> Written {
        Object::end(self)
    }
}

/// An open JSON array (a sequence, tuple, or the array of a tuple variant).
struct Array<'w> {
    writer: &'w mut Writer,
    empty: bool,
    close_variant: bool,
}

impl Array<'_> {
    fn element<T: Serialize + ?Sized>(&mut self, value: &T) -> Written {
        if !self.empty {
            self.writer.out.push(b',');
        }
        self.empty = false;
        value.serialize(&mut *self.writer)
    }

    fn end(self) -> Written {
        self.writer.out.push(b']');
        if self.close_variant {
            self.writer.out.push(b'}');
        }
        Ok(())
    }
}

macro_rules! array_impls {
    ($($trait:ident :: $method:ident),*) => {$(
        impl ser::$trait for Array<'_> {
            type Ok = ();
            type Error = Unsupported;

            fn $method<T: Serialize + ?Sized>(&mut self, value: &T) -> Written {
                self.element(value)
            }

            fn end(self) -> Written {
                Array::end(self)
            }
        }
    )*};
}

array_impls!(
    SerializeSeq::serialize_element,
    SerializeTuple::serialize_element,
    SerializeTupleStruct::serialize_field,
    SerializeTupleVariant::serialize_field
);

impl<'w> ser::Serializer for &'w mut Writer {
    type Ok = ();
    type Error = Unsupported;
    type SerializeSeq = Array<'w>;
    type SerializeTuple = Array<'w>;
    type SerializeTupleStruct = Array<'w>;
    type SerializeTupleVariant = Array<'w>;
    type SerializeMap = Object<'w>;
    type SerializeStruct = Object<'w>;
    type SerializeStructVariant = Object<'w>;

    fn serialize_bool(self, value: bool) -> Written {
        self.out.extend_from_slice(if value { b"true" } else { b"false" });
        Ok(())
    }

    fn serialize_i8(self, value: i8) -> Written {
        self.serialize_i64(i64::from(value))
    }

    fn serialize_i16(self, value: i16) -> Written {
        self.serialize_i64(i64::from(value))
    }

    fn serialize_i32(self, value: i32) -> Written {
        self.serialize_i64(i64::from(value))
    }

    fn serialize_i64(self, value: i64) -> Written {
        self.push_digits(value.unsigned_abs(), value < 0);
        Ok(())
    }

    fn serialize_u8(self, value: u8) -> Written {
        self.serialize_u64(u64::from(value))
    }

    fn serialize_u16(self, value: u16) -> Written {
        self.serialize_u64(u64::from(value))
    }

    fn serialize_u32(self, value: u32) -> Written {
        self.serialize_u64(u64::from(value))
    }

    fn serialize_u64(self, value: u64) -> Written {
        self.push_digits(value, false);
        Ok(())
    }

    fn serialize_i128(self, value: i128) -> Written {
        // `to_value` keeps what fits 64 bits and refuses the rest.
        match (u64::try_from(value), i64::try_from(value)) {
            (Ok(value), _) => self.serialize_u64(value),
            (_, Ok(value)) => self.serialize_i64(value),
            _ => Err(Unsupported),
        }
    }

    fn serialize_u128(self, value: u128) -> Written {
        self.serialize_u64(u64::try_from(value).map_err(|_| Unsupported)?)
    }

    fn serialize_f32(self, _value: f32) -> Written {
        // `to_value` widens it through its own rules; the reference path writes it.
        Err(Unsupported)
    }

    fn serialize_f64(self, value: f64) -> Written {
        // A double prints like JS; JSON.stringify writes non-finite numbers as null.
        if value.is_finite() {
            self.out.extend_from_slice(units::format_number(value).as_bytes());
        } else {
            self.out.extend_from_slice(b"null");
        }
        Ok(())
    }

    fn serialize_char(self, value: char) -> Written {
        self.push_string(value.encode_utf8(&mut [0; 4]));
        Ok(())
    }

    fn serialize_str(self, value: &str) -> Written {
        self.push_string(value);
        Ok(())
    }

    fn serialize_bytes(self, value: &[u8]) -> Written {
        let mut array = ser::Serializer::serialize_seq(self, Some(value.len()))?;
        for byte in value {
            ser::SerializeSeq::serialize_element(&mut array, byte)?;
        }
        array.end()
    }

    fn serialize_none(self) -> Written {
        self.serialize_unit()
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Written {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Written {
        self.out.extend_from_slice(b"null");
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Written {
        self.serialize_unit()
    }

    fn serialize_unit_variant(self, _name: &'static str, _index: u32, variant: &'static str) -> Written {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _name: &'static str, value: &T) -> Written {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Written {
        self.out.push(b'{');
        self.push_string(variant);
        self.out.push(b':');
        value.serialize(&mut *self)?;
        self.out.push(b'}');
        Ok(())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Written<Array<'w>> {
        self.out.push(b'[');
        Ok(Array { writer: self, empty: true, close_variant: false })
    }

    fn serialize_tuple(self, len: usize) -> Written<Array<'w>> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Written<Array<'w>> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Written<Array<'w>> {
        self.out.push(b'{');
        self.push_string(variant);
        self.out.extend_from_slice(b":[");
        Ok(Array { writer: self, empty: true, close_variant: true })
    }

    fn serialize_map(self, _len: Option<usize>) -> Written<Object<'w>> {
        Ok(self.begin_object())
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Written<Object<'w>> {
        Ok(self.begin_object())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Written<Object<'w>> {
        self.out.push(b'{');
        self.push_string(variant);
        self.out.push(b':');
        let mut object = self.begin_object();
        object.close_variant = true;
        Ok(object)
    }
}

/// Writes a map key, quoted: strings, characters, integers and unit variants, which
/// `serde_json::to_value` turns into the same strings. Anything else goes to the reference path.
struct KeyWriter<'w> {
    writer: &'w mut Writer,
}

impl KeyWriter<'_> {
    fn digits(self, value: u64, negative: bool) -> Written<KeyRef> {
        let out = &mut self.writer.out;
        out.push(b'"');
        let start = out.len();
        push_digits(out, value, negative);
        let key = KeyRef { rank: key_rank(&out[start..]), start: position(start)?, len: key_len(out.len() - start)? };
        out.push(b'"');
        Ok(key)
    }
}

impl ser::Serializer for KeyWriter<'_> {
    type Ok = KeyRef;
    type Error = Unsupported;
    type SerializeSeq = ser::Impossible<KeyRef, Unsupported>;
    type SerializeTuple = ser::Impossible<KeyRef, Unsupported>;
    type SerializeTupleStruct = ser::Impossible<KeyRef, Unsupported>;
    type SerializeTupleVariant = ser::Impossible<KeyRef, Unsupported>;
    type SerializeMap = ser::Impossible<KeyRef, Unsupported>;
    type SerializeStruct = ser::Impossible<KeyRef, Unsupported>;
    type SerializeStructVariant = ser::Impossible<KeyRef, Unsupported>;

    fn serialize_str(self, value: &str) -> Written<KeyRef> {
        self.writer.push_key(value)
    }

    fn serialize_char(self, value: char) -> Written<KeyRef> {
        self.serialize_str(value.encode_utf8(&mut [0; 4]))
    }

    fn serialize_i8(self, value: i8) -> Written<KeyRef> {
        self.serialize_i64(i64::from(value))
    }

    fn serialize_i16(self, value: i16) -> Written<KeyRef> {
        self.serialize_i64(i64::from(value))
    }

    fn serialize_i32(self, value: i32) -> Written<KeyRef> {
        self.serialize_i64(i64::from(value))
    }

    fn serialize_i64(self, value: i64) -> Written<KeyRef> {
        self.digits(value.unsigned_abs(), value < 0)
    }

    fn serialize_u8(self, value: u8) -> Written<KeyRef> {
        self.serialize_u64(u64::from(value))
    }

    fn serialize_u16(self, value: u16) -> Written<KeyRef> {
        self.serialize_u64(u64::from(value))
    }

    fn serialize_u32(self, value: u32) -> Written<KeyRef> {
        self.serialize_u64(u64::from(value))
    }

    fn serialize_u64(self, value: u64) -> Written<KeyRef> {
        self.digits(value, false)
    }

    fn serialize_unit_variant(self, _name: &'static str, _index: u32, variant: &'static str) -> Written<KeyRef> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _name: &'static str, value: &T) -> Written<KeyRef> {
        value.serialize(self)
    }

    fn serialize_bool(self, _value: bool) -> Written<KeyRef> {
        Err(Unsupported)
    }

    fn serialize_i128(self, _value: i128) -> Written<KeyRef> {
        Err(Unsupported)
    }

    fn serialize_u128(self, _value: u128) -> Written<KeyRef> {
        Err(Unsupported)
    }

    fn serialize_f32(self, _value: f32) -> Written<KeyRef> {
        Err(Unsupported)
    }

    fn serialize_f64(self, _value: f64) -> Written<KeyRef> {
        Err(Unsupported)
    }

    fn serialize_bytes(self, _value: &[u8]) -> Written<KeyRef> {
        Err(Unsupported)
    }

    fn serialize_none(self) -> Written<KeyRef> {
        Err(Unsupported)
    }

    fn serialize_some<T: Serialize + ?Sized>(self, _value: &T) -> Written<KeyRef> {
        Err(Unsupported)
    }

    fn serialize_unit(self) -> Written<KeyRef> {
        Err(Unsupported)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Written<KeyRef> {
        Err(Unsupported)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Written<KeyRef> {
        Err(Unsupported)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Written<Self::SerializeSeq> {
        Err(Unsupported)
    }

    fn serialize_tuple(self, _len: usize) -> Written<Self::SerializeTuple> {
        Err(Unsupported)
    }

    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Written<Self::SerializeTupleStruct> {
        Err(Unsupported)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Written<Self::SerializeTupleVariant> {
        Err(Unsupported)
    }

    fn serialize_map(self, _len: Option<usize>) -> Written<Self::SerializeMap> {
        Err(Unsupported)
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Written<Self::SerializeStruct> {
        Err(Unsupported)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Written<Self::SerializeStructVariant> {
        Err(Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn sorts_keys_and_puts_index_keys_first() {
        let value = json!({"b": 1, "a": [1, 2.5, "x"], "10": true, "2": null, "01": 0});
        let expected = r#"{"2":null,"10":true,"01":0,"a":[1,2.5,"x"],"b":1}"#;
        assert_eq!(stringify_value(&value), expected);
        assert_eq!(stringify_value_tree(&value), expected);
    }

    #[test]
    fn key_ranks_order_like_compare_keys() {
        let keys = [
            "",
            "\u{0}",
            "a",
            "a\u{0}",
            "a\u{0}b",
            "ab",
            "abcdefg",
            "abcdefg\u{0}",
            "abcdefgh",
            "abcdefgh\u{0}",
            "abcdefghi",
            "abcdefgZ",
            "abcdefgAA",
            "abcdef\u{e9}",
            "abcdefg\u{e9}",
            "0",
            "1",
            "10",
            "4294967294",
            "4294967295",
            "01",
            "-1",
            "Z",
            "\u{e9}",
            "\u{FFFF}",
            "\u{1F600}",
            "a\u{1F600}",
            "\u{1F600}\u{1F600}\u{1F600}\u{1F600}",
            "soilFertility",
            "soilMoisture",
            "soilState",
            "type",
            "x",
            "y",
        ];
        for a in keys {
            // The rank's definition, written out slowly.
            let slow: u128 = if let Some(index) = array_index(a) {
                u128::from(index)
            } else {
                let units: Vec<u16> = a.encode_utf16().collect();
                let prefix = (0..7).fold(0u128, |p, i| p << 16 | u128::from(units.get(i).copied().unwrap_or(0)));
                1 << 127 | prefix << 4 | units.len().min(8) as u128
            };
            assert_eq!(key_rank(a.as_bytes()), slow, "{a:?}");
            for b in keys {
                let (ra, rb) = (key_rank(a.as_bytes()), key_rank(b.as_bytes()));
                if ra != rb {
                    assert_eq!(ra.cmp(&rb), compare_keys(a, b), "{a:?} {b:?}");
                } else if rank_is_exact(ra) {
                    assert_eq!(a, b);
                }
                assert_eq!(compare_key_bytes(a.as_bytes(), b.as_bytes()), compare_keys(a, b));
            }
        }
    }

    #[test]
    fn recognises_array_indices() {
        assert_eq!(array_index("0"), Some(0));
        assert_eq!(array_index("4294967294"), Some(4294967294));
        assert_eq!(array_index("4294967295"), None);
        assert_eq!(array_index("01"), None);
        assert_eq!(array_index(""), None);
    }

    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    enum Shape {
        Unit,
        Newtype(u8),
        Tuple(i8, String),
        Struct { z: bool, a: Option<f64> },
    }

    #[derive(serde::Serialize)]
    struct Unordered {
        zebra: Vec<Shape>,
        #[serde(rename = "\u{FFFF}")]
        high: u8,
        #[serde(rename = "\u{1F600}")]
        emoji: u8,
        apple: (u8, char),
        #[serde(rename = "7")]
        seven: (),
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
        int_keys: BTreeMap<i32, &'static str>,
        numbers: Value,
    }

    #[test]
    fn writes_what_the_reference_writes() {
        let mut extra = BTreeMap::new();
        extra.insert("apple".to_owned(), json!("flattened duplicate wins"));
        extra.insert("10".to_owned(), json!({"q\"\n\u{1}": [f64::MAX, -0.0, 1e21, 1.5e-7]}));
        let value = Unordered {
            zebra: vec![
                Shape::Unit,
                Shape::Newtype(3),
                Shape::Tuple(-4, "t\\".to_owned()),
                Shape::Struct { z: true, a: Some(0.1) },
                Shape::Struct { z: false, a: None },
            ],
            high: 1,
            emoji: 2,
            apple: (9, 'c'),
            seven: (),
            extra,
            int_keys: [(-1, "minus"), (0, "zero"), (12, "twelve"), (3, "three")].into_iter().collect(),
            numbers: json!({"u": u64::MAX, "i": i64::MIN, "big": 9007199254740993_u64}),
        };
        let fast = stringify(&value);
        assert_eq!(fast, stringify_via_value(&value));
        assert!(fast.starts_with(r#"{"7":null,"10":"#), "{fast}");
    }

    #[test]
    fn falls_back_for_what_it_leaves_to_the_reference() {
        let float_keys: BTreeMap<bool, f32> = [(true, 0.1_f32), (false, 2.0)].into_iter().collect();
        assert_eq!(stringify(&float_keys), stringify_via_value(&float_keys));
        assert_eq!(stringify(&(u128::MAX >> 70)), "288230376151711743");
        assert_eq!(stringify(&(i128::MIN >> 70)), stringify_via_value(&(i128::MIN >> 70)));
        assert!(try_stringify(&u128::MAX).is_err());
    }
}
