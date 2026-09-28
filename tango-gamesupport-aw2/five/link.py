#!/usr/bin/env python3
"""Link one relocatable Thumb object (clang --target=thumbv4t-none-eabi -c)
at a fixed GBA ROM address, without a real linker.

Usage: link.py <in.o> <base-hex> <out.bin> <out-symbols.rs> [NAME=0xADDR ...]

All allocated PROGBITS sections (.text*, .rodata*) are laid out in order from
<base>; .bss/.data are refused (the helpers keep their state in fixed RAM).
Undefined symbols are resolved from NAME=0xADDR arguments (game functions and
data). Handles R_ARM_THM_CALL (bl), R_ARM_THM_JUMP24/11/8, R_ARM_ABS32,
R_ARM_REL32 and R_ARM_THM_PC8-style literal loads within the blob.
The symbols file lists every global function as a Rust `pub const`.
"""
import struct, sys

def u32(b, o): return struct.unpack_from('<I', b, o)[0]
def u16(b, o): return struct.unpack_from('<H', b, o)[0]

def main():
    src, base, out_bin, out_rs = sys.argv[1], int(sys.argv[2], 16), sys.argv[3], sys.argv[4]
    ext = {}
    for a in sys.argv[5:]:
        k, v = a.split('=')
        ext[k] = int(v, 16)
    elf = open(src, 'rb').read()
    shoff = u32(elf, 0x20); shentsize = u16(elf, 0x2E); shnum = u16(elf, 0x30); shstrndx = u16(elf, 0x32)
    secs = []
    for i in range(shnum):
        o = shoff + i * shentsize
        name, typ, flags, addr, off, size, link, info, align, entsize = struct.unpack_from('<IIIIIIIIII', elf, o)
        secs.append(dict(name=name, type=typ, flags=flags, off=off, size=size, link=link, info=info, align=max(align, 1), entsize=entsize))
    strtab = secs[shstrndx]
    def sname(n):
        s = strtab['off'] + n
        return elf[s:elf.index(b'\0', s)].decode()
    for s in secs: s['nm'] = sname(s['name'])
    # layout
    blob = bytearray(); place = {}
    for i, s in enumerate(secs):
        if s['flags'] & 2 == 0 or s['size'] == 0 or s['nm'].startswith('.ARM.exidx'):
            continue
        if s['type'] == 8:  # NOBITS
            sys.exit(f'refusing .bss section {s["nm"]}: keep state in fixed RAM')
        if s['flags'] & 1 and s['nm'].startswith('.data'):
            sys.exit(f'refusing writable section {s["nm"]}')
        while len(blob) % s['align']: blob.append(0)
        place[i] = base + len(blob)
        blob += elf[s['off']:s['off'] + s['size']]
    # symbols
    symsec = next(s for s in secs if s['type'] == 2)
    strs = secs[symsec['link']]
    syms = []
    for j in range(symsec['size'] // 16):
        o = symsec['off'] + j * 16
        nm, val, size, info, other, shndx = struct.unpack_from('<IIIBBH', elf, o)
        s0 = strs['off'] + nm
        name = elf[s0:elf.index(b'\0', s0)].decode()
        syms.append((name, val, size, info, shndx))
    def symaddr(k):
        name, val, size, info, shndx = syms[k]
        if shndx == 0:
            if name not in ext: sys.exit(f'undefined symbol {name}')
            return ext[name]
        if shndx == 0xFFF1: return val
        if shndx not in place: sys.exit(f'symbol {name} in unplaced section')
        return place[shndx] + val
    # relocations
    for s in secs:
        if s['type'] != 9: continue  # SHT_REL
        tgt = s['info']
        if tgt not in place: continue
        for j in range(s['size'] // 8):
            r_off, r_info = struct.unpack_from('<II', elf, s['off'] + j * 8)
            typ, k = r_info & 0xFF, r_info >> 8
            S = symaddr(k)
            P = place[tgt] + r_off
            at = P - base
            name = syms[k][0]
            thumb = syms[k][4] != 0 and syms[k][3] & 0xF == 2  # STT_FUNC defined here
            if typ == 2:  # ABS32
                A = u32(blob, at)
                v = S + A
                if thumb: v |= 1
                struct.pack_into('<I', blob, at, v & 0xFFFFFFFF)
            elif typ == 3:  # REL32
                A = u32(blob, at)
                struct.pack_into('<I', blob, at, (S + A - P) & 0xFFFFFFFF)
            elif typ in (10, 30):  # THM_CALL / THM_JUMP24 -> bl (ARMv4T has no b.w)
                hi, lo = u16(blob, at), u16(blob, at + 2)
                if typ == 30: sys.exit(f'THM_JUMP24 to {name} at {P:#x}: compile with -mlong-calls off or avoid tail calls (-fno-optimize-sibling-calls)')
                off = (S & ~1) - (P + 4)
                if not -(1 << 22) <= off < (1 << 22): sys.exit(f'bl to {name} out of range from {P:#x}')
                off >>= 1
                struct.pack_into('<HH', blob, at, 0xF000 | ((off >> 11) & 0x7FF), 0xF800 | (off & 0x7FF))
            elif typ == 102:  # THM_JUMP11
                ins = u16(blob, at)
                off = ((S & ~1) - (P + 4)) >> 1
                struct.pack_into('<H', blob, at, (ins & 0xF800) | (off & 0x7FF))
            elif typ == 103:  # THM_JUMP8
                ins = u16(blob, at)
                off = ((S & ~1) - (P + 4)) >> 1
                struct.pack_into('<H', blob, at, (ins & 0xFF00) | (off & 0xFF))
            elif typ == 11:  # THM_PC8 (ldr rX, [pc, #imm])
                ins = u16(blob, at)
                off = (S - ((P + 4) & ~3)) >> 2
                struct.pack_into('<H', blob, at, (ins & 0xFF00) | (off & 0xFF))
            else:
                sys.exit(f'unhandled relocation type {typ} for {name} at {P:#x}')
    open(out_bin, 'wb').write(blob)
    with open(out_rs, 'w') as f:
        f.write('// Generated by five/link.py: addresses of the helpers in five.bin.\n')
        f.write(f'pub const BASE: u32 = {base:#010x};\n')
        for k, (name, val, size, info, shndx) in enumerate(syms):
            if info >> 4 == 1 and shndx not in (0, 0xFFF1) and shndx in place:
                a = place[shndx] + val
                f.write(f'pub const {name.upper()}: u32 = {a:#010x};\n')
    print(f'{len(blob)} bytes at {base:#x}')

main()
