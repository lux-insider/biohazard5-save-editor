/* ECC das páginas da NAND do Xbox 360: checkEcc da Free60 (NAND File System),
   com uint32_t no lugar de unsigned long. Referência independente para os
   testes do extrair-kv (o programa em Rust e o script em Python).

   Lê páginas de 0x210 bytes (512 de dados + 16 de spare) da entrada padrão e
   imprime, para cada uma, 1 ou 0 (o ECC gravado confere ou não) e os 4 bytes
   de ECC calculados. */
#include <stdint.h>
#include <stdio.h>

static void calcular(const uint8_t *pagina, uint8_t ecc[4]) {
    unsigned int i, val = 0, v = 0;
    const uint32_t *dados = (const uint32_t *)pagina;
    for (i = 0; i < 0x1066; i++) {
        if (!(i & 31)) {
            if (i == 0x1000) dados = (const uint32_t *)(pagina + 0x200);
            v = ~*dados++;
        }
        val ^= v & 1;
        v >>= 1;
        if (val & 1) val ^= 0x6954559;
        val >>= 1;
    }
    val = ~val;
    ecc[0] = (val << 6) & 0xC0;
    ecc[1] = (val >> 2) & 0xFF;
    ecc[2] = (val >> 10) & 0xFF;
    ecc[3] = (val >> 18) & 0xFF;
}

int main(void) {
    uint8_t pagina[0x210], ecc[4];
    while (fread(pagina, 1, sizeof pagina, stdin) == sizeof pagina) {
        calcular(pagina, ecc);
        int confere = (pagina[0x20C] & 0xC0) == ecc[0] && pagina[0x20D] == ecc[1]
                      && pagina[0x20E] == ecc[2] && pagina[0x20F] == ecc[3];
        printf("%d %02x%02x%02x%02x\n", confere, ecc[0], ecc[1], ecc[2], ecc[3]);
    }
    return 0;
}
