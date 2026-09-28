#!/usr/bin/env python3
"""Turn five/patches.txt into src/five_patches.rs, checking every original
instruction against the ROM (the .gba is only read here, never changed).

Usage: gen.py <rom.gba>
"""
import os, re, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REGS = ['r0', 'r1', 'r2', 'r3', 'r4', 'r5', 'r6', 'r7']


def imm(v):
    return f'#{v}' if v < 10 else f'#0x{v:x}'


def decode(h):
    """The Thumb forms the patches touch, in the listing's syntax."""
    if h >> 13 == 0 and (h >> 11) & 3 != 3:
        op = ['lsls', 'lsrs', 'asrs'][(h >> 11) & 3]
        return f'{op} {REGS[h & 7]}, {REGS[(h >> 3) & 7]}, {imm((h >> 6) & 31)}'
    if h >> 11 == 3:
        sub, isimm = (h >> 9) & 1, (h >> 10) & 1
        op = 'subs' if sub else 'adds'
        n = (h >> 6) & 7
        return f'{op} {REGS[h & 7]}, {REGS[(h >> 3) & 7]}, ' + (imm(n) if isimm else REGS[n])
    if h >> 13 == 1:
        op = ['movs', 'cmp', 'adds', 'subs'][(h >> 11) & 3]
        return f'{op} {REGS[(h >> 8) & 7]}, {imm(h & 0xFF)}'
    if h >> 10 == 0x10:
        ops = ['ands', 'eors', 'lsls', 'lsrs', 'asrs', 'adcs', 'sbcs', 'rors', 'tst', 'rsbs', 'cmp', 'cmn', 'orrs', 'muls', 'bics', 'mvns']
        return f'{ops[(h >> 6) & 15]} {REGS[h & 7]}, {REGS[(h >> 3) & 7]}'
    if h & 0xF000 in (0x7000, 0x8000) or h & 0xF000 == 0x6000:
        kind = {0x6000: ('str', 'ldr', 4), 0x7000: ('strb', 'ldrb', 1), 0x8000: ('strh', 'ldrh', 2)}[h & 0xF000]
        op = kind[1] if h & 0x800 else kind[0]
        off = ((h >> 6) & 31) * kind[2]
        rb, rd = REGS[(h >> 3) & 7], REGS[h & 7]
        return f'{op} {rd}, [{rb}]' if off == 0 else f'{op} {rd}, [{rb}, {imm(off)}]'
    if h & 0xF800 == 0x4800:
        return f'ldr {REGS[(h >> 8) & 7]}, [pc, {imm((h & 0xFF) * 4)}]'
    if h & 0xF600 == 0xB400:
        lst = [REGS[i] for i in range(8) if h >> i & 1]
        if h & 0x100:
            lst.append('lr' if not h & 0x800 else 'pc')
        return ('pop' if h & 0x800 else 'push') + ' {' + ', '.join(lst) + '}'
    return f'?{h:04x}'


def encode_imm(h, new):
    if h >> 13 == 1:
        assert 0 <= new <= 0xFF
        return (h & 0xFF00) | new
    if h >> 11 == 3 and (h >> 10) & 1:
        assert 0 <= new <= 7
        return (h & ~(7 << 6)) | (new << 6)
    raise ValueError(f'no immediate in {h:04x}')


KINDS = ['a0', 'a0_id4', 'a0_id4_hi16', 'a0_hi16', 'a0_hi24', 'a0_shl6', 'b', 's', 'mul51', 'mul102', 'mul612', 'cur_base', 'bitidx', 'parity']


def camel(s):
    return ''.join(p.capitalize() for p in s.split('_'))


def main():
    rom = open(sys.argv[1], 'rb').read()
    h16 = lambda a: struct.unpack_from('<H', rom, a - 0x08000000)[0]
    w32 = lambda a: struct.unpack_from('<I', rom, a - 0x08000000)[0]
    imms, hooks, words, halves, errors = [], [], [], [], []
    seen = set()
    for n, line in enumerate(open(os.path.join(HERE, 'patches.txt')), 1):
        if line.lstrip().startswith('#'):
            continue
        if ' # ' in line:
            line = line.split(' # ', 1)[0]
        if '|' in line:
            spec, want = (s.strip() for s in line.split('|', 1))
        else:
            spec, want = line.strip(), None
        if not spec:
            continue
        f = spec.split()
        kind, addr = f[0], int(f[1], 16)
        if addr in seen:
            errors.append(f'line {n}: {addr:08x} patched twice')
        seen.add(addr)
        if kind in ('imm', 'hook', 'fn', 'pre'):
            have = decode(h16(addr))
            if want is not None and have != want:
                errors.append(f'line {n}: {addr:08x} is `{have}`, spec says `{want}`')
                continue
        if kind == 'imm':
            old = h16(addr)
            imms.append((addr, old, encode_imm(old, int(f[2], 0))))
        elif kind == 'hook':
            k, dst, src = f[2], REGS.index(f[3]), REGS.index(f[4])
            assert k in KINDS, k
            hooks.append((addr, h16(addr), f'Hook::Set(Op::{camel(k)}, {dst}, {src})'))
        elif kind in ('fn', 'pre'):
            what = 'Replace' if kind == 'fn' else 'Before'
            hooks.append((addr, h16(addr), f'Hook::{what}(Routine::{camel(f[2])})'))
        elif kind == 'word':
            words.append((addr, w32(addr), f[2]))
        elif kind == 'half':
            old, new = int(f[2], 0), int(f[3], 0)
            if h16(addr) != old:
                errors.append(f'line {n}: {addr:08x} holds {h16(addr):04x}, spec says {old:04x}')
            halves.append((addr, old, new))
    if errors:
        print('\n'.join(errors))
        sys.exit(1)
    out = os.path.join(HERE, '..', 'src', 'five_patches.rs')
    with open(out, 'w') as o:
        o.write('// Generated from five/patches.txt by five/gen.py; do not edit.\n')
        o.write('use crate::five::{Hook, Op, Routine, Table};\n\n')
        o.write('/// (address, original halfword, patched halfword)\n')
        o.write('pub const HALVES: &[(u32, u16, u16)] = &[\n')
        for a, old, new in imms + halves:
            o.write(f'    (0x{a:08X}, 0x{old:04X}, 0x{new:04X}),\n')
        o.write('];\n\n/// (address, original word, table it points to instead)\n')
        o.write('pub const WORDS: &[(u32, u32, Table)] = &[\n')
        for a, old, t in words:
            o.write(f'    (0x{a:08X}, 0x{old:08X}, Table::{camel(t.lower())}),\n')
        o.write('];\n\n/// (address, original halfword, what runs there)\n')
        o.write('pub const HOOKS: &[(u32, u16, Hook)] = &[\n')
        for a, old, hk in hooks:
            o.write(f'    (0x{a:08X}, 0x{old:04X}, {hk}),\n')
        o.write('];\n')
    print(f'{len(imms) + len(halves)} halfwords, {len(words)} words, {len(hooks)} hooks -> {os.path.relpath(out)}')


main()
