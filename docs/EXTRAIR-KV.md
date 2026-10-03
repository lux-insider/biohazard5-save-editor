# Como tirar o kv.bin do seu console

O **kv.bin** (keyvault) guarda o número de série, os certificados e as chaves que pertencem só ao seu Xbox 360. Com ele, o editor assina o save do Xbox, e o save sai pronto para o console.

O keyvault fica **criptografado** dentro da NAND e só abre com a **CPU key** do próprio console. O programa `extrair-kv`, que vem junto com o editor para Linux e Windows, lê a cópia da NAND e decifra o keyvault. Antes de gravar o `kv.bin`, ele **confere o KV do mesmo jeito que o console confere**.

O `extrair-kv` é escrito em Rust e não precisa de nada instalado. O script em Python (`ferramentas/extrair-kv.py`) continua no repositório e faz exatamente o mesmo, para quem preferir.

> Tudo acontece no seu computador, sem internet, e a CPU key nunca aparece na tela.
>
> O `kv.bin`, a CPU key e a cópia da NAND são a **identidade do seu console**. **Nunca mande** esses arquivos para ninguém, nem coloque no GitHub ou na nuvem sem senha.

---

## Do começo ao fim

| Onde | O que fazer | O que sai |
|---|---|---|
| **1. No console** | fazer a cópia da NAND com o Simple 360 NAND Flasher | `flashdmp.bin`, `cpukey.txt` e o log, no pendrive |
| **2. No PC** | tirar o kv.bin com o `extrair-kv` | `kv.bin` |
| **3. No PC** | pôr a chave no editor | o save do Xbox sai assinado |

### Parte 1: no console

**O que você precisa:**
- um console **desbloqueado (RGH ou JTAG)** que abra homebrew, por exemplo pelo Aurora, pelo FSD ou pelo XeXMenu;
- um **pendrive** com espaço livre: uns 70 MB bastam, ou 600 MB para copiar inteira uma NAND big block de 512 MB;
- o **Simple 360 NAND Flasher**, homebrew do Swizzy.

**Passos:**

1. Copie a pasta do Simple 360 NAND Flasher para o pendrive. **Não deixe nenhum `updflash.bin` nessa pasta.** Com ele lá, o menu também mostra as opções que **gravam** na NAND.
2. No console, abra o `default.xex` dessa pasta pelo gerenciador de arquivos (Aurora, FSD, XeXMenu…).
3. O programa detecta a memória do console: aparece "Detected MMC NAND device!" no eMMC ou "Detected RAW NAND device!" numa NAND comum. Depois ele tenta ler a CPU key ("Attempting to grab CPUKey..."). Se conseguir, mostra "Your CPUKey is: …" e grava o `cpukey.txt`.
4. No menu, **aperte só o X** ("Press X if you want to dump your nand with Rawdump…"). Qualquer outro botão fecha o programa.

   > ⚠️ Se aparecerem as opções **A** ("flash") e **B** ("safeflash"), elas **gravam** uma imagem na NAND do console. Para fazer a cópia, **nunca aperte A nem B nesse menu.**

5. Numa NAND **big block** (Jasper de 256 ou 512 MB), ele pergunta o tamanho da cópia. **A** copia só o sistema, 64 MB, e é o recomendado. **B** copia a NAND inteira. Nessa pergunta, o A e o B só escolhem o tamanho da cópia.
6. **Espere sem mexer no console nem no controle**, até aparecer "Done! successfully dumped …" (eMMC) ou "NAND Dumped! :D" (NAND comum). No eMMC de 48 MB leva cerca de 40 segundos. Depois aparece "Press any button to exit!".
7. Leve o pendrive ao PC e copie o `flashdmp.bin`, o `cpukey.txt` e o `Simple 360 NAND Flasher.log`. Depois **apague os três do pendrive**, porque o `cpukey.txt` e o log têm a CPU key.

**Mensagens que podem aparecer:**

| Mensagem | O que fazer |
|---|---|
| `WARNING: game:\flashdmp.bin already exists!` | Já existe uma cópia na pasta, e "Press Start" grava por cima. Se a cópia anterior ainda não foi para o PC, saia sem apertar Start e copie antes. |
| `Your dashboard is to old for this feature, sorry... use xell!` | O programa não conseguiu ler a CPU key nesse painel. A cópia da NAND funciona normalmente; a CPU key você vê no XeLL. |
| `SFCX: Bad block found at …` | Um bloco da NAND está marcado como ruim. O `extrair-kv` diz depois se o KV veio inteiro. |
| `failed to dump NAND :(` ou `ERROR: Unable to open …` | A cópia não foi feita. Confira o espaço livre no pendrive e tente de novo, ou use outro pendrive. |

**Para ter certeza (opcional):** faça a cópia duas vezes, levando a primeira para o PC antes da segunda, e compare as duas. Iguais quer dizer que a leitura foi confiável. Isso é importante se um dia você for gravar essa cópia de volta no console.

```bash
sha256sum flashdmp-1.bin flashdmp-2.bin                 # Linux
certutil -hashfile flashdmp-1.bin SHA256                 # Windows, um de cada vez
```

### Parte 2: no PC, tirar o kv.bin

1. Pegue o `extrair-kv` na página de **Releases**, junto com o editor: `extrair-kv` no Linux, `extrair-kv.exe` no Windows. Não precisa instalar nada.
2. **Abra o menu**, sem digitar nada:
   - **Windows:** dois cliques no `extrair-kv.exe`;
   - **Linux:** `./extrair-kv` no terminal, na pasta onde ele está. Se o Linux disser que não tem permissão, rode `chmod +x extrair-kv` uma vez.

   O menu procura as cópias da NAND em Downloads, na Área de Trabalho, em Documentos e nos pendrives. Ele **reconhece cada cópia pelo conteúdo**, então o nome do arquivo não importa, e acha a NAND também dentro de zip. Para cada cópia, mostra o tipo e, se a CPU key estiver junto, **de qual console ela é** (série e peça):

   ```
     [1] meu xbox da sala (backup velho).zip
         em ~/Downloads
         eMMC de 4 GB, cópia de 48 MB (Corona 4 GB, Winchester)
         série 123456789012 · peça X123456-001 · CPU key confere

     [2] flashdmp.bin
         em E:\Simple 360 NAND Flasher
         NAND de 16 MB, com ECC (Xenon a Jasper, Trinity, Corona 16 MB)
         sem a CPU key junto: ela vai ser pedida

     [c] digitar ou arrastar para cá o caminho de outra cópia (zip, pasta ou arquivo)
     [s] sair
   ```

   Escolha o número. Se a CPU key não estiver junto, o menu pede para você digitar, e ela não aparece na tela. Depois ele pergunta onde gravar:
   - **na pasta do editor** (`console/kv.bin`): se o editor estiver em um desses lugares, ele já aparece na lista;
   - **ao lado da cópia**, como `KV-<série>.bin`, para não misturar os KVs de consoles diferentes;
   - ou em outra pasta.

   Ele nunca grava por cima de um arquivo. No fim, mostra onde gravou e espera um ENTER para fechar.

   > O menu não abre cópias dentro de um **7z com senha**. Extraia a cópia antes.

3. **Quem prefere digitar o comando** também pode, na pasta onde estão o `flashdmp.bin` e o `cpukey.txt`:
   ```bash
   ./extrair-kv flashdmp.bin cpukey.txt               # Linux
   extrair-kv.exe flashdmp.bin cpukey.txt             # Windows, no Prompt de Comando
   ```
4. Confira se apareceu **CPU key confere** e se a **série** é a mesma da etiqueta do console. Os detalhes estão em [Como usar](#como-usar) e [O que aparece na tela](#o-que-aparece-na-tela), mais abaixo.

### Parte 3: no PC, pôr a chave no editor

| Jeito | Precisa compilar? | Como |
|---|---|---|
| **Versão normal** (o recomendado, no Linux e no Windows) | não | Grave o KV como `console/kv.bin` dentro da pasta do editor: no menu, escolha "na pasta do editor"; no comando, use `-o "<pasta do editor>/console/kv.bin"`. Ao abrir um save do Xbox, o editor assina sozinho. |
| **Versão GOLD** (a chave fica dentro do programa) | sim, no Linux | Siga a seção [Versão GOLD](../README.md#versão-gold-a-chave-do-seu-console-embutida) do README. Cada pessoa compila a sua e **nunca compartilha o programa**. |

Por fim, guarde a cópia da NAND, a CPU key e o kv.bin com senha, como explicado em [Guardar com segurança](#guardar-com-segurança).

---

## Com quais consoles funciona

Com a cópia de **qualquer placa**, de **qualquer tamanho**. O programa descobre sozinho o tipo da cópia.

| Cópia | Placas | Como o programa lê |
|---|---|---|
| **eMMC de 4 GB**, cópia de 48 MB ou a imagem inteira | Corona 4 GB, Winchester | direto, sem ECC |
| **NAND de 16 MB** com ECC | Trinity, Corona 16 MB, Xenon a Jasper | tira os 16 bytes de ECC de cada página e confere cada um |
| **NAND de 64, 256 ou 512 MB** com ECC | Jasper big block | igual à de 16 MB |
| Qualquer uma acima **sem os bytes de ECC** | todas | direto |

Ele lê só o começo da cópia, onde fica o keyvault, e não o arquivo inteiro. Até a imagem de 4 GB abre em menos de 1 segundo.

Foi conferido com cópias reais de um **Trinity** (NAND de 16 MB com ECC) e de um **Winchester** (eMMC de 48 MB). Os outros formatos foram conferidos com cópias de teste (veja "Testes", no fim). O `extrair-kv` em Rust foi comparado lado a lado com o script em Python, que leu essas cópias reais: nos mesmos casos de teste, ele grava o mesmo kv.bin, mostra as mesmas telas do menu e as mesmas mensagens, menos três que ficaram mais claras (zip que não é zip, zip com senha e o nome do programa na dica).

---

## O que você precisa

| O quê | De onde vem |
|---|---|
| a cópia da NAND | `flashdmp.bin`, do Simple 360 NAND Flasher, ou `nanddump.bin`, do J-Runner |
| a CPU key | `cpukey.txt`, gerado pelos mesmos programas |
| o `extrair-kv` | vem junto com o editor, para Linux e Windows. Ou o `ferramentas/extrair-kv.py`, com Python 3.8 ou mais novo. |

A NAND e a CPU key podem estar soltas, numa pasta ou dentro de um `.zip`.

### O que o Simple 360 NAND Flasher grava

Ele roda no próprio console (RGH ou JTAG) e grava tudo **na mesma pasta do programa**, no pendrive ou no HD:

| O console tem | Modo do programa | O que sai no `flashdmp.bin` |
|---|---|---|
| **eMMC de 4 GB** (Corona 4 GB, Winchester) | "MMC/Corona v2 [4GB] Mode", rawdump4g | a área do sistema, **48 MB, sem ECC** |
| **NAND de 16 MB** (Trinity, Corona 16 MB e anteriores) | "NAND Mode", rawdump v1 | 16 MB **com o ECC** de cada página |
| **NAND big block de 256 ou 512 MB** (Jasper) | pergunta: **A** = só o sistema, **B** = tudo | **A:** 64 MB com ECC (o recomendado); **B:** 256 ou 512 MB com ECC |

| Arquivo | O que é |
|---|---|
| `cpukey.txt` | a CPU key, lida do console |
| `Simple 360 NAND Flasher.log` | o registro do que ele fez. **Também tem a CPU key**, na linha "Your CPUKey is:", então merece o mesmo cuidado. |
| `recovery.bin` | a cópia que ele faz antes de gravar, quando você escolhe "safeflash" |
| `updflash.bin` | a imagem que você coloca para ele gravar no console. Não é backup. |

---

## Como usar

```
extrair-kv   ORIGEM   [CPUKEY]   [opções]
```

No Linux, rode `./extrair-kv` na pasta dele; no Windows, `extrair-kv.exe`. O script em Python aceita os mesmos argumentos: `python3 ferramentas/extrair-kv.py ...` (no Windows, `py` no lugar de `python3`).

| Jeito | Comando |
|---|---|
| **menu** (acha as cópias sozinho) | `extrair-kv`, sem mais nada, ou dois cliques no `extrair-kv.exe` no Windows |
| direto do zip do backup | `extrair-kv backup-nand.zip` |
| arquivos soltos | `extrair-kv flashdmp.bin cpukey.txt` |
| a pasta do backup | `extrair-kv pasta-do-backup` |
| só a NAND | `extrair-kv flashdmp.bin` |

- **Zip e pasta:** a NAND é o arquivo que começa com os bytes `FF 4F`. O programa prefere os nomes `flashdmp`, `recovery`, `nanddump` e `nand`. O `updflash.bin` fica por último.
- **Zip:** sem compressão ou com a compressão normal (deflate), também zip64 e o .exe que se extrai sozinho. É o que o Windows, o 7-Zip, o WinRAR e os apps de celular fazem por padrão.
- **A CPU key:** o programa olha os arquivos com "cpu" e "key" no nome e os `.txt` e `.log` pequenos. Ele testa cada sequência de 32 caracteres hexadecimais que achar e usa a que abre o keyvault. Pode haver outras chaves no mesmo arquivo, como a DVD key.
- **Só a NAND:** procura a CPU key na mesma pasta. Se não achar, **pede para você digitar**, e o que você digita não aparece na tela.

| Opção | O que faz |
|---|---|
| `-o ARQUIVO` | onde gravar. Sem ela, grava `KV.bin` na pasta atual. Pode ser uma pasta, e a pasta é criada se não existir. |
| `--gravar-danificado` | grava mesmo se o KV estiver danificado. **Só para tentar recuperar algo**, nunca para usar. |
| `--version`, `-h` | a versão e a ajuda |

### Para usar no editor

- **Versão normal:** grave direto na pasta `console`, ao lado do programa:
  ```bash
  extrair-kv backup-nand.zip -o console/kv.bin
  ```
  No Linux, o nome tem que ser `kv.bin`, em minúsculas.
- **Versão GOLD (chave embutida):** grave fora do repositório e siga o README, em [Versão GOLD](../README.md#versão-gold-a-chave-do-seu-console-embutida):
  ```bash
  cargo run --release -p extrair-kv -- ~/Downloads/backup-nand.zip -o ~/kv-temp/kv.bin
  RE5_KV_EMBUTIDO=~/kv-temp/kv.bin cargo build --release --features kv-embutido --target-dir target-pessoal
  ```

---

## O que aparece na tela

Exemplo com valores inventados:

```
NAND:      flashdmp.bin (16 MB)
Tipo:      NAND de 16 MB, com ECC (Xenon a Jasper, Trinity, Corona 16 MB)
Keyvault:  posição 0x4000, lida do cabeçalho da NAND
ECC:       32 de 32 páginas do KV conferem
CPU key:   confere (HMAC-SHA1 igual ao que o console calcula)
Série:     123456789012   <- tem que ser igual ao da etiqueta do console
Peça:      X123456-001
Gravado:   /home/voce/KV.bin (16384 bytes), só você pode ler
SHA-256:   (64 caracteres)
```

| Linha | O que quer dizer |
|---|---|
| Tipo | o tipo de cópia que o programa reconheceu |
| Keyvault | onde o KV estava. O endereço vem do cabeçalho da própria NAND. |
| ECC | só nas NANDs com ECC: quantas das 32 páginas do KV passaram na conferência de leitura |
| CPU key | a conferência principal: o HMAC-SHA1 bateu, do mesmo jeito que o console confere |
| Série e Peça | o número de série e o número da peça do console, gravados no KV |
| SHA-256 | a "impressão digital" do arquivo, para conferir as cópias depois. Anote. |

**Como saber se deu certo:**
1. **`CPU key: confere`** quer dizer duas coisas: a CPU key é deste console, e o keyvault está inteiro, sem nenhum byte trocado.
2. **A Série** tem que ser igual à da etiqueta atrás do console, ou à de **Configurações → Sistema → Configurações do console → Informações do sistema**.

---

## Mensagens de erro

| Mensagem | O que fazer |
|---|---|
| `já existe ...` | O programa nunca grava por cima. Mova ou renomeie o arquivo antigo, ou use `-o` com outro nome. |
| `o keyvault não abriu com a CPU key` | A CPU key não é deste console, ou a cópia não é de uma NAND do Xbox 360. **Nada foi gravado.** |
| `a CPU key é deste console, mas o keyvault da cópia está danificado` | A chave está certa, mas algum byte do KV veio errado. **Faça outra cópia da NAND.** Se a mensagem disser quantas páginas estão com o ECC errado, foi erro de leitura do chip. |
| `não achei a CPU key` | Passe o arquivo dela depois da NAND, ou rode num terminal para o programa pedir. |
| `isso não é uma CPU key` | O que foi digitado não tem 32 caracteres de 0 a 9 e de A a F. |
| `não achei uma cópia da NAND` | O zip ou a pasta não tem nenhum arquivo que comece com `FF 4F`. |
| `AVISO: a cópia não começa com FF 4F` | O arquivo pode não ser uma NAND. Se a CPU key conferir, está tudo bem. |
| `a cópia é pequena demais para ter o keyvault` | O arquivo está cortado antes do KV. Faça outra cópia. |
| `o zip tem senha` | Extraia os arquivos e rode com eles soltos. |
| `o zip usa um tipo de compressão que este programa não lê` | O zip foi feito com uma compressão diferente (LZMA, BZip2, Deflate64). Extraia os arquivos, com o 7-Zip por exemplo, e rode com eles soltos. |
| `o zip está estragado` ou `o arquivo não é um zip válido` | O zip veio cortado ou com defeito. Copie ou baixe de novo, ou extraia o que der e rode com os arquivos soltos. |
| `não achei: ...` | O caminho está errado. Confira o nome e as aspas. |

---

## Guardar com segurança

Junte o backup e o `kv.bin` num 7z com senha e apague as cópias soltas:

```bash
7z a -p -mhe=on backup-console.7z backup-nand.zip KV.bin
7z t backup-console.7z && rm KV.bin
```

- `-p` pede uma senha. **Sem ela, não tem como abrir.**
- `-mhe=on` esconde também os nomes dos arquivos.
- `7z t ... && rm KV.bin` só apaga o arquivo solto se o teste passar.
- Guarde o 7z em **dois lugares**. Anote o SHA-256 do KV para conferir as cópias com `sha256sum KV.bin`.

---

## Como funciona

1. **Tipo da cópia:** se o tamanho é múltiplo de 528 bytes, a cópia tem ECC: são 512 bytes de dados e 16 de spare em cada página. Se não for, é sem ECC, como a do eMMC. Se o primeiro jeito não abrir o KV, o programa tenta o outro.
2. **Posição do KV:** o cabeçalho da NAND guarda o tamanho do keyvault na posição `0x60` e o endereço na posição `0x6C`. Em todos os consoles conhecidos, o endereço é `0x4000`. Se o cabeçalho estiver estranho, o programa usa `0x4000`.
3. **ECC:** nas NANDs com ECC, confere os 26 bits de ECC de cada uma das 32 páginas do KV, com o algoritmo da Free60.
4. **Decifrar:** os 16 primeiros bytes do KV não são criptografados. A chave do RC4 sai deles junto com a CPU key: `HMAC-SHA1(CPU key, 16 primeiros bytes)`, usando os 16 primeiros bytes do resultado. O resto do KV é decifrado com essa chave.
5. **Conferir:** calcula `HMAC-SHA1(CPU key, resto do KV + 07 12)`. Os 16 primeiros bytes têm que ser iguais aos 16 primeiros bytes do KV. É a mesma conta do `kv_read` do libxenon, a base do XeLL.
6. **KV danificado:** se o HMAC não bate, mas a série, o certificado e a chave privada do console (p × q = n) estão certos, a CPU key é a certa e algum byte da cópia veio errado.
7. **Gravar:** grava sem nunca sobrescrever, só para o dono, em modo binário, e lê de volta para conferir. Se algo der errado no meio, apaga o arquivo pela metade.

### Segurança do programa

- **Rust seguro:** a parte do `extrair-kv` que lê a NAND, o zip e as CPU keys não tem nenhum `unsafe` (`#![forbid(unsafe_code)]`). No programa todo, o `unsafe` aparece só em três chamadas ao console do Windows, para as cores e para ler a CPU key sem mostrar.
- **Contas conferidas:** o programa é compilado com a conferência de estouro ligada também na versão final: uma conta que estoura fecha o programa, em vez de seguir com um valor errado.
- **Zip conferido:** cada posição e cada tamanho lido do zip é comparado com o tamanho do arquivo antes de usar, e nada é lido além do necessário. Um zip estragado vira uma mensagem de erro. Os testes cortam e embaralham zips milhares de vezes para conferir isso.
- **Leve:** um executável de uns 600 KB, sem instalar nada. Lê só o começo da cópia (1,1 MB), até de dentro do zip.

### Testes

```bash
cargo test --release -p extrair-kv                   # o programa em Rust
python3 ferramentas/testes/test_extrair_kv.py        # o script em Python
```

Os testes montam keyvaults e NANDs de todos os formatos com uma CPU key inventada. O ECC das cópias de teste é calculado pelo `ferramentas/testes/ecc_free60.c`, a referência da Free60, compilado na hora. O menu e a CPU key digitada são testados num terminal de verdade. Os testes também conferem que a CPU key nunca aparece na tela.

Os testes do Rust fazem tudo o que os do Python fazem e mais: zips feitos pelo zip do Info-ZIP (também zip64), pelo 7-Zip e pelo Python, zip com senha de verdade, com uma compressão que o programa não lê, com dados antes (o .exe que se extrai sozinho), e zips cortados e embaralhados.

### Fontes

- Free60, NAND File System (cabeçalho, ECC e spare): https://github.com/Free60Project/wiki/blob/master/docs/System-Software/NAND_File_System.md
- libxenon, `kv_read` e as posições dentro do keyvault: https://github.com/Free60Project/libxenon/blob/master/libxenon/drivers/xb360/xb360.c
- libxenon, tipos de NAND e tamanho da página: https://github.com/Free60Project/libxenon/blob/master/libxenon/drivers/xenon_nand/xenon_sfcx.c
