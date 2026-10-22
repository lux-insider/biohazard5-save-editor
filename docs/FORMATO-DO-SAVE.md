# Formato do save do Resident Evil 5

Referência técnica do que o editor lê e grava. Os valores vêm de
`src/save.rs`, e cada um foi conferido em saves reais e no jogo.

## Visão geral

O Xbox 360 e o PC guardam o mesmo arquivo interno, o `savedata.bin`. O que
muda é o "envelope" em volta dele:

| | Xbox 360 | PC (Steam) |
|---|---|---|
| Arquivo | pacote STFS **CON** com o `savedata.bin` dentro | o próprio `savedata.bin` |
| Ordem dos bytes | big-endian | little-endian |
| Criptografia do `savedata.bin` | nenhuma | XOR encadeado em blocos de 8 bytes |
| Integridade ao gravar | checksum + hashes SHA-1 do pacote + assinatura RSA | checksum |
| Pasta | `Content\<XUID>\434307D4\00000001\` | `Steam\userdata\<conta>\21690\remote\` |

Title ID do jogo: `434307D4`.

## savedata.bin

### Cabeçalho

| Offset | Tamanho | Campo |
|---|---|---|
| `0x04` | 4 | fim da segunda região do checksum (`0x4BC0` no Xbox) |
| `0x08` | 4 | checksum |
| `0x10` | — | início dos dados |

### Checksum

Soma de 32 bits (com estouro) das palavras de `[0x10, fim)`, com
`fim = 0x4BC0` no Xbox e `0x52B0` no PC.

- **Xbox 360:** todas as palavras entram, inclusive a de `0x3BAC`. O jogo
  grava `0x00020000` ali depois que a munição infinita é ligada no jogo;
  um save regravado sem somar essa palavra aparece como corrompido.
- **PC:** a palavra em `0x3BAC` fica de fora (formato do editor de PC em
  que este se baseou).

Somar o Xbox até `0x52B0` só funciona
enquanto o trecho `[0x4BC0, 0x52B0)` é todo zero, o que acontece apenas no
começo do jogo. Um save com mais progresso e o checksum calculado assim
aparece no jogo como "Os dados salvos parecem estar corrompidos".

### Endereços

Os campos do PC ficam deslocados em relação aos do Xbox:

| Campo | Xbox 360 | PC | Formato |
|---|---|---|---|
| Gold / Money | `0xE0` | `0x194` | u32 |
| Listas de desbloqueio | `0x70` | `0x124` | bits (ver abaixo) |
| Baú (inventário) | `0x3158`, 84 espaços | `0x384C`, 74 espaços | 12 bytes por espaço; ID e quantidade em u16 |
| Slots do Chris | `0x3690` | `0x3D80` | 9 slots de `0x2C` bytes; ID e quantidade em u32 |
| Slots da Sheva | `0x3AB0` | `0x41A0` | 9 slots de `0x2C` bytes |

### Listas de desbloqueio

Cada lista é um campo de bits a partir do início do bloco de listas; o bit N
é o item N contado do fim da lista. O editor altera só os bits conhecidos e
preserva os outros.

| Deslocamento | Lista | Itens |
|---|---|---|
| `+0x00` | Roupas do Chris | 4 |
| `+0x04` | Roupas da Sheva | 4 |
| `+0x08` | Filtros de tela | 4 |
| `+0x0C` | Munição infinita | 19 armas |
| `+0x10` | Arquivos da Biblioteca | 12 |
| `+0x14` | Figuras | 45 (64 bits: palavra baixa, depois alta) |

### Criptografia do PC

O arquivo do PC é cifrado com XOR em blocos de 8 bytes. O primeiro bloco usa
a chave inicial `00 21 11 08 C0 4B 00 00`; cada bloco seguinte usa o bloco
**cifrado** anterior como chave.

## Pacote CON (Xbox 360)

O pacote STFS do tipo CON é assinado pelo console. Campos usados pelo editor:

| Offset | Campo |
|---|---|
| `0x344` | tipo de conteúdo (`1` = save de jogo) |
| `0x360` | Title ID |
| `0x36C` | Console ID (5 bytes) |
| `0x371` | Perfil / XUID (8 bytes) |
| `0x379` | descritor do volume STFS |
| `0x3FD` | Device ID (20 bytes) |
| `0x411` | nome exibido no dashboard (9 idiomas, `0x100` bytes cada) |
| `0x171A` | miniatura do save (PNG) |
| `0x571A` | miniatura do jogo (PNG) |

Ao gravar, o editor:

1. recalcula o checksum do `savedata.bin`;
2. regrava os blocos do arquivo e refaz os hashes SHA-1 de cada bloco, da
   tabela de hashes, do diretório e do cabeçalho;
3. assina o cabeçalho com RSA (PKCS#1 v1.5, SHA-1), com o `kv.bin` do console
   ou com a chave interna do programa.

Pacotes LIVE e PIRS têm outro cabeçalho e outra assinatura e são recusados.

## Perfil e gamertag

O perfil do Xbox é um pacote CON (tipo `0x10000`, Title ID `FFFE07D1`). A
gamertag está no arquivo interno `Account`, cifrado com RC4 e conferido por
HMAC-SHA1. O editor só **lê** o perfil, decifra em memória, usa apenas a
gamertag e guarda somente o par `XUID → gamertag`:

- Linux: `~/.local/share/re5-save-editor-360-pc/gamertags.json`
- Windows: `%APPDATA%\re5-save-editor-360-pc\gamertags.json`

## Gravação segura

1. Cópia do arquivo atual para `backups/`.
2. Gravação numa cópia ao lado do destino (`<arquivo>.gravando`), com
   `fsync`.
3. Leitura de volta e conferência completa (checksum, hashes e assinatura).
4. Só então a cópia substitui o original com uma troca de nome atômica.

Se qualquer etapa falhar, o original não é tocado e o temporário é removido.
