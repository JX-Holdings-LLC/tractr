// The draw-list shader of farm-gpu: every command is an instanced quad in device pixels whose
// fragment computes the same premultiplied colour as the CPU rasterizer (farm-render's raster.rs),
// with its integer arithmetic where the CPU has it. Blending happens in the pipeline (source-over,
// modulate or replace).

struct Target {
    size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> target_info: Target;
// Glyph coverage (R8).
@group(0) @binding(1) var glyphs: texture_2d<f32>;
// Shapes the CPU rasterized (premultiplied RGBA8).
@group(0) @binding(2) var sprites: texture_2d<f32>;
// Per-frame lookup rows (R32Uint): 0 = upscale columns, 1 = upscale rows, 2.. = tint colours per
// native row (packed premultiplied RGBA, 0 = transparent).
@group(0) @binding(3) var lut: texture_2d<u32>;
// The image of image draws (premultiplied RGBA8), or the native world for the upscale.
@group(1) @binding(0) var image: texture_2d<f32>;

const SOLID: u32 = 0u;
const AA_RECT: u32 = 1u;
const ROUND_RECT: u32 = 2u;
const IMAGE_NEAREST: u32 = 3u;
const IMAGE_SMOOTH: u32 = 4u;
const GLYPH: u32 = 5u;
const SPRITE: u32 = 6u;
const TINT: u32 = 7u;
const UPSCALE: u32 = 8u;
const COPY: u32 = 9u;

struct Instance {
    // The quad: left, top, right, bottom in device pixels.
    @location(0) rect: vec4<f32>,
    // kind, colour (RGBA8 packed, straight alpha), tint (0 = none), extra (opacity byte, LUT row…)
    @location(1) data: vec4<u32>,
    @location(2) p0: vec4<f32>,
    @location(3) p1: vec4<f32>,
};

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) data: vec4<u32>,
    @location(1) @interpolate(flat) p0: vec4<f32>,
    @location(2) @interpolate(flat) p1: vec4<f32>,
    @location(3) @interpolate(flat) origin: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32, instance: Instance) -> Varyings {
    // Two triangles: (0, 1, 2) and (2, 1, 3) of the corners top-left, top-right, bottom-left,
    // bottom-right.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vertex];
    let point = mix(instance.rect.xy, instance.rect.zw, corner);
    let clip = vec2<f32>(point.x / target_info.size.x * 2.0 - 1.0, 1.0 - point.y / target_info.size.y * 2.0);
    var out: Varyings;
    out.position = vec4<f32>(clip, 0.0, 1.0);
    out.data = instance.data;
    out.p0 = instance.p0;
    out.p1 = instance.p1;
    out.origin = instance.rect.xy;
    return out;
}

// Skia's low-precision div255: (v + 255) >> 8.
fn div255(v: u32) -> u32 {
    return (v + 255u) >> 8u;
}

// SkMulDiv255Round: a * b / 255 rounded to nearest.
fn mul255(a: u32, b: u32) -> u32 {
    let product = a * b + 128u;
    return (product + (product >> 8u)) >> 8u;
}

fn channel(color: u32, index: u32) -> u32 {
    return (color >> (index * 8u)) & 255u;
}

// The CPU's `premultiplied(color, coverage)`.
fn premultiplied(color: u32, coverage: u32) -> vec4<f32> {
    let alpha = min(div255(channel(color, 3u) * coverage + 127u), 255u);
    return vec4<f32>(
        f32(mul255(channel(color, 0u), alpha)),
        f32(mul255(channel(color, 1u), alpha)),
        f32(mul255(channel(color, 2u), alpha)),
        f32(alpha),
    ) / 255.0;
}

fn unpack(packed: u32) -> vec4<f32> {
    return vec4<f32>(f32(channel(packed, 0u)), f32(channel(packed, 1u)), f32(channel(packed, 2u)), f32(channel(packed, 3u))) / 255.0;
}

fn bytes(texel: vec4<f32>) -> vec4<u32> {
    return vec4<u32>(round(clamp(texel, vec4<f32>(0.0), vec4<f32>(1.0)) * 255.0));
}

// An image texel (premultiplied bytes) through the tint (the CPU's tinted copy) and the paint
// alpha (`scale_1_float`).
fn paint_texel(texel: vec4<u32>, tint: u32, paint: u32) -> vec4<f32> {
    // `paint`: the opacity byte, and bit 8 set when there is a tint.
    let alpha = paint & 255u;
    var c = texel;
    if ((paint & 256u) != 0u) {
        let ta = channel(tint, 3u);
        let factors = vec4<u32>(mul255(channel(tint, 0u), ta), mul255(channel(tint, 1u), ta), mul255(channel(tint, 2u), ta), ta);
        c = vec4<u32>(mul255(c.x, factors.x), mul255(c.y, factors.y), mul255(c.z, factors.z), mul255(c.w, factors.w));
    }
    if (alpha < 255u) {
        c = vec4<u32>(div255(c.x * alpha), div255(c.y * alpha), div255(c.z * alpha), div255(c.w * alpha));
    }
    return vec4<f32>(c) / 255.0;
}

// One axis of Skia's nearest-neighbour mapping (raster.rs `NearestAxis::sample`).
fn nearest(device: f32, axis: vec4<f32>) -> i32 {
    let u = (device + 0.5) * axis.x + axis.y;
    let length = i32(axis.w);
    return i32(axis.z) + min(i32(max(u, 0.0)), length - 1);
}

fn load_clamped(position: vec2<f32>, low: vec2<i32>, high: vec2<i32>) -> vec4<f32> {
    return textureLoad(image, clamp(vec2<i32>(floor(position)), low, high), 0);
}

@fragment
fn fs_main(in: Varyings) -> @location(0) vec4<f32> {
    let pixel = floor(in.position.xy);
    let center = pixel + vec2<f32>(0.5);
    let kind = in.data.x;
    let color = in.data.y;
    switch kind {
        case SOLID: {
            return premultiplied(color, 255u);
        }
        case AA_RECT: {
            let covered = clamp(min(pixel + vec2<f32>(1.0), in.p0.zw) - max(pixel, in.p0.xy), vec2<f32>(0.0), vec2<f32>(1.0));
            let coverage = u32(covered.x * covered.y * 255.0 + 0.5);
            if (coverage == 0u) {
                discard;
            }
            return premultiplied(color, coverage);
        }
        case ROUND_RECT: {
            let q = abs(center - in.p0.xy) - in.p0.zw;
            let radius = in.p1.x;
            var distance: f32;
            if (q.x > 0.0 && q.y > 0.0) {
                distance = sqrt(q.x * q.x + q.y * q.y) - radius;
            } else {
                distance = max(q.x, q.y) - radius;
            }
            let coverage = clamp(0.5 - distance, 0.0, 1.0);
            if (coverage <= 0.0) {
                discard;
            }
            return premultiplied(color, u32(coverage * 255.0 + 0.5));
        }
        case IMAGE_NEAREST: {
            let at = vec2<i32>(nearest(pixel.x, in.p0), nearest(pixel.y, in.p1));
            return paint_texel(bytes(textureLoad(image, at, 0)), in.data.z, in.data.w);
        }
        case IMAGE_SMOOTH: {
            // Bilinear filtering of the cropped source, clamped at its edges.
            let u = center * vec2<f32>(in.p0.x, in.p0.z) + vec2<f32>(in.p0.y, in.p0.w) - vec2<f32>(0.5);
            let low = vec2<i32>(in.p1.xy);
            let high = vec2<i32>(in.p1.zw) - vec2<i32>(1);
            let base = floor(u);
            let f = u - base;
            let a = load_clamped(base, low, high);
            let b = load_clamped(base + vec2<f32>(1.0, 0.0), low, high);
            let c = load_clamped(base + vec2<f32>(0.0, 1.0), low, high);
            let d = load_clamped(base + vec2<f32>(1.0, 1.0), low, high);
            let texel = mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
            return paint_texel(bytes(texel), in.data.z, in.data.w);
        }
        case GLYPH: {
            let at = vec2<i32>(in.p0.xy + pixel - in.origin);
            let coverage = u32(round(textureLoad(glyphs, at, 0).x * 255.0));
            if (coverage == 0u) {
                discard;
            }
            return premultiplied(color, coverage);
        }
        case SPRITE: {
            let at = vec2<i32>(in.p0.xy + pixel - in.origin);
            return textureLoad(sprites, at, 0);
        }
        case TINT: {
            let packed = textureLoad(lut, vec2<i32>(i32(pixel.y), i32(in.data.w)), 0).x;
            if (packed == 0u) {
                discard;
            }
            return unpack(packed);
        }
        case UPSCALE: {
            let offset = vec2<i32>(pixel - in.p0.xy);
            let column = textureLoad(lut, vec2<i32>(offset.x, 0), 0).x;
            let row = textureLoad(lut, vec2<i32>(offset.y, 1), 0).x;
            return textureLoad(image, vec2<i32>(i32(column), i32(row)), 0);
        }
        case COPY: {
            return textureLoad(image, vec2<i32>(pixel - in.p0.xy), 0);
        }
        default: {
            discard;
        }
    }
    return vec4<f32>(0.0);
}
