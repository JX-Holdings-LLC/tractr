#!/usr/bin/env python3
"""Generate the author's font coverage check from the game's actual Inter font cmap."""
from pathlib import Path
import struct
import sys

root = Path(__file__).resolve().parents[2]
font = (root / 'assets/fonts/Inter-Regular.ttf').read_bytes()
u16 = lambda at: struct.unpack_from('>H', font, at)[0]
u32 = lambda at: struct.unpack_from('>I', font, at)[0]
cmap = next(u32(at + 8) for at in range(12, 12 + 16 * u16(4), 16) if font[at:at + 4] == b'cmap')
points = set()
for record in range(cmap + 4, cmap + 4 + 8 * u16(cmap + 2), 8):
    platform, encoding = u16(record), u16(record + 2)
    if platform != 0 and (platform != 3 or encoding not in (1, 10)):
        continue
    table = cmap + u32(record + 4)
    form = u16(table)
    if form == 4:
        count = u16(table + 6) // 2
        ends = table + 14
        starts = ends + count * 2 + 2
        deltas = starts + count * 2
        offsets = deltas + count * 2
        for i in range(count):
            for point in range(u16(starts + i * 2), u16(ends + i * 2) + 1):
                offset = u16(offsets + i * 2)
                glyph = u16(offsets + i * 2 + offset + (point - u16(starts + i * 2)) * 2) if offset else point
                if not offset or glyph:
                    glyph = (glyph + u16(deltas + i * 2)) & 0xffff
                if glyph and point != 0xffff:
                    points.add(point)
    elif form == 12:
        for at in range(table + 16, table + 16 + 12 * u32(table + 12), 12):
            start, end, glyph = u32(at), u32(at + 4), u32(at + 8)
            points.update(range(start + (glyph == 0), end + 1))
ranges = []
for point in sorted(points):
    if ranges and point == ranges[-1][1] + 1:
        ranges[-1][1] = point
    else:
        ranges.append([point, point])
lines = ['namespace FarmEngine.Authoring', '', '/// Generated from assets/fonts/Inter-Regular.ttf by tools/codegen/font-coverage.py.', 'module internal FontCoverage =', '    let private ranges = [|']
lines += [f'        0x{start:X}, 0x{end:X}' for start, end in ranges]
lines += ['    |]', '', '    let firstMissing (text: string) =', '        text.EnumerateRunes()', '        |> Seq.tryFind (fun rune ->', '            not (System.Text.Rune.IsWhiteSpace rune || System.Text.Rune.IsControl rune)', '            && not (ranges |> Array.exists (fun (first, last) -> rune.Value >= first && rune.Value <= last)))', '']
output = root / 'src/FarmEngine.Authoring/FontCoverage.fs'
expected = '\n'.join(lines)
if '--check' in sys.argv:
    if output.read_text() != expected:
        sys.exit('FontCoverage.fs is stale: run python3 tools/codegen/font-coverage.py')
else:
    output.write_text(expected)
