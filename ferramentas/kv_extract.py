#!/usr/bin/env python3
"""Extrai e decifra o keyvault (kv.bin) de um dump de NAND do Xbox 360.

Uso: python3 kv_extract.py flashdmp.bin cpukey.txt kv.bin

Funciona com NAND de 16 MB (paginas de 0x200 + 0x10 de ECC). Confere o
resultado procurando o certificado do console (tamanho 0x1A8) e mostra so o
part number -- nunca imprime a CPU key.
"""
import hashlib
import hmac
import struct
import sys


def rc4(key, data):
    s = list(range(256))
    j = 0
    for i in range(256):
        j = (j + s[i] + key[i % len(key)]) & 255
        s[i], s[j] = s[j], s[i]
    i = j = 0
    out = bytearray()
    for b in data:
        i = (i + 1) & 255
        j = (j + s[i]) & 255
        s[i], s[j] = s[j], s[i]
        out.append(b ^ s[(s[i] + s[j]) & 255])
    return bytes(out)


def main(nand_path, cpukey_path, out_path):
    raw = open(nand_path, 'rb').read()
    nand = b''.join(raw[i:i + 0x200] for i in range(0, len(raw), 0x210))  # tira o ECC
    kv_off = struct.unpack_from('>I', nand, 0x60)[0]   # normalmente 0x4000
    enc = nand[kv_off:kv_off + 0x4000]
    cpukey = bytes.fromhex(open(cpukey_path).read().strip())
    key = hmac.new(cpukey, enc[:0x10], hashlib.sha1).digest()[:0x10]
    kv = enc[:0x10] + rc4(key, enc[0x10:])
    cert = kv[0x9C8:0x9C8 + 0x1A8]
    if struct.unpack_from('>H', cert)[0] != 0x1A8:
        sys.exit('falhou: CPU key nao corresponde a esta NAND')
    open(out_path, 'wb').write(kv)
    print('kv.bin gravado | part number do console:', cert[7:0x12].decode('ascii', 'replace'))


if __name__ == '__main__':
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    main(*sys.argv[1:])
