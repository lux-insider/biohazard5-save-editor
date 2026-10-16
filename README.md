# BIOHAZARD 5 SAVE EDITOR

Editor de save do **Resident Evil 5** para **Xbox 360** e **PC (Steam)** num
programa só, em português. Feito em Rust com interface em [Slint](https://slint.dev),
para Linux e Windows, sem HTML e sem depender de navegador ou WebView.

![Tela inicial](docs/prints/inicio.jpg)

## Novidades da 3.0.0

### O save do Xbox sai assinado sem precisar do kv.bin

- O programa traz uma **chave interna de assinatura**. O save do Xbox 360 sai
  sempre **assinado e pronto para o console**, sem o keyvault do seu Xbox:
  não precisa de RGH/JTAG, cópia da NAND nem do `extrair-kv`.
- **Conferido no console**: um save reassinado com a chave interna carregou
  normalmente no RE5.
- Se você tiver o keyvault do seu console, ele continua sendo usado. Ordem das
  chaves ao gravar:
  1. `console/kv.bin` ao lado do programa (o keyvault do seu console);
  2. o kv.bin embutido, se compilado com `--features kv-embutido` (GOLD);
  3. a **chave interna** (sempre existe).
- O rodapé da aba Save diz qual chave vai assinar.

### Checksum do Xbox corrigido (importante)

- **No Xbox, o checksum do savedata.bin termina em `0x4BC0`, não em
  `0x52B0`.** As versões anteriores somavam até `0x52B0` (o fim do PC). Isso
  só dava certo enquanto o trecho `[0x4BC0, 0x52B0)` era todo zero, o que
  acontece em saves do começo do jogo. Com mais progresso esse trecho ganha
  dados, o editor gravava um checksum que o jogo não aceita e o RE5 dizia
  **"Os dados salvos parecem estar corrompidos"**.
- `0x4BC0` é o valor da palavra em `0x4` do savedata.bin e fica `0x6F0` antes
  do fim do PC, a mesma diferença dos outros endereços. O PC continua com
  `0x52B0`.
- **Conferido no console**, com um save novo gravado pelo próprio Xbox:
  - o checksum que o console gravou só bate com o fim em `0x4BC0`;
  - o save só reassinado: **carregou**;
  - o save com o checksum da fórmula antiga: **"corrompido"**;
  - o save com edições (munição infinita da M92F e munição de lança-granadas
    nos slots do Chris e da Sheva) e o checksum novo: **carregou**, com os
    itens no jogo.
- Saves que foram gravados por versões anteriores e têm dados depois de
  `0x4BC0` aparecem agora com **checksum "não confere"**. Abrir e salvar de
  novo nesta versão corrige.

### Aba Save nova

Cabe inteira na janela normal, sem rolar:

- **Cabeçalho** (Xbox): as **miniaturas do save e do jogo**, o **nome que
  aparece no dashboard** (por exemplo "RESIDENT EVIL 5 SAVEDATA") e, numa
  linha, o jogo, o **Title ID lido do próprio arquivo** e o **Console ID**.
  Embaixo, o nome e o caminho do arquivo. À direita, a **data do save**.
- **Gold e Exchange Points** (no PC, "Money"): valores acima de 9.999.999 (o
  máximo do jogo) mostram um **aviso em laranja**, que não impede de salvar.
- **Perfil** (Xbox; no PC, "Conta"): perfil (XUID) com a **gamertag**, device
  ID e a **pasta do pendrive** onde o save tem que ficar
  (`Content\<XUID>\434307D4\00000001\`). Ela muda na hora quando você troca o
  perfil ou usa "Copiar de outro save…".
- **Integridade**: os selos de **Checksum do jogo**, **Hashes do pacote** e
  **Assinatura** (com o console que assinou). Uma explicação curta, em
  vermelho, aparece só quando algo não confere.
- Ao fundo, a arte do **elenco do The Mercenaries**, bem apagada, sumindo em
  degradê para a esquerda. São os 10 recortes dos personagens da ISO
  (`MerceResource.arc`, `merce_11` a `merce_19` e `merce_21`), montados pelo
  próprio layout da tela do Mercenaries, na qualidade original.

### Gamertag do perfil (Xbox)

- O rótulo do XUID mostra a **gamertag**, por exemplo "Perfil (XUID) ·
  ABadAvatar", e acompanha o XUID da tela.
- **Aparece sozinha ao abrir o save** quando o perfil está no lugar padrão,
  ao lado do save no mesmo pendrive ou HD:
  `Content\<XUID>\FFFE07D1\00010000\<XUID>`.
- Se o perfil estiver em outro lugar, use **LER PERFIL…** e escolha o arquivo
  do perfil uma vez: a gamertag fica guardada para todos os saves daquele XUID.
- O perfil é um pacote CON (tipo `0x10000`, Title ID `FFFE07D1`); a gamertag
  está no arquivo interno `Account`, cifrado com RC4 e conferido por
  HMAC-SHA1. O editor só **lê** o perfil, nunca grava nele.
- **Privacidade:** o `Account` também guarda dados sensíveis da conta (senha
  da Passport, passcode, chave online). O editor decifra em memória e usa só
  a gamertag; o arquivo `gamertags.json` guarda apenas `XUID → gamertag`
  (Linux: `~/.local/share/re5-save-editor-360-pc/`; Windows: `%APPDATA%`).

### Renomear o save no dashboard (Xbox)

- Botão **RENOMEAR** ao lado do nome do save: troca o nome que aparece no
  dashboard do Xbox (de 1 a 128 letras, com acentos). **DESFAZER** volta o
  nome original.
- O nome vai **só para os idiomas que já têm nome** no save, como o jogo e o
  console deixaram. Se nenhum tiver nome, vai para os 9 idiomas principais.
- Fica no cabeçalho do pacote: ao salvar, o hash do cabeçalho e a assinatura
  são refeitos. O savedata.bin não muda.

### Mais segurança ao abrir e gravar

- **Só abre saves do Resident Evil 5**: o Title ID tem que ser `434307D4` e o
  tipo tem que ser "save de jogo". Outro arquivo dá uma mensagem clara.
- **Só pacotes CON** (assinados por console). LIVE/PIRS são recusados em vez
  de saírem incoerentes ao assinar.
- **Assinatura conferida por inteiro**, não só o final.
- **Bloco do diretório** incluído na verificação de hashes e recalculado ao
  salvar (num save íntegro o valor não muda).
- **Arquivo corrompido não fecha o programa**: um número de bloco fora da
  tabela vira mensagem de erro.
- **Conteúdos extras (DLC) continuam só para consulta.** Eles são carregados
  na memória do jogo e mexer neles no savedata.bin pode travar o jogo; nada
  disso é gravado (o teste `nada_novo_fica_gravavel` garante).

### Visual e aba História

- O amarelo saiu: títulos e destaques são **letras brancas**. A **textura de
  sangue** da seleção (a mesma da aba escolhida) fica só no nome do save e no
  título e nos cabeçalhos da aba História.
- O aviso de save gravado começa com **★** (a estrela da fonte do jogo; o ✔
  não existe nela e aparecia como ◆).
- **História: a rolagem agora vai até o fim do texto.** Em textos longos a
  barra parava antes do final (em "Sheva Alomar", na página 28 de 36): com
  cada página num bloco de layout dentro da repetição, o Slint calculava a
  altura total menor do que a real. Agora cada página é um texto simples na
  lista e o separador é um item próprio. Ao escolher outro texto, a leitura
  volta para o começo.

## Novidades da 2.1.0

![Save de PC (Steam) aberto](docs/prints/pc-steam.jpg)

- **Inventário no save de PC (Steam).** As abas PERSONAGENS (os 9 slots do
  Chris e da Sheva) e INVENTÁRIO (o baú) agora funcionam também no save de
  PC, não só no do Xbox 360. Os endereços foram achados num save real da
  Steam e conferidos slot por slot contra a tela do jogo.
- **Conferido no jogo.** Num save real da Steam, o dinheiro e os pontos
  editados apareceram no RE5.
- **Abre direto na pasta da Steam**: `userdata/<conta>/21690/remote`, no
  Windows e no Linux (nativo, Flatpak e Snap).

## Novidades da 2.0.0

O editor ganhou a cara do jogo. Tudo o que aparece na tela veio dos arquivos
do próprio *BIOHAZARD 5*: texturas, layouts dos menus e a fonte.

- **Novo nome: BIOHAZARD 5 SAVE EDITOR.**
- **Tela inicial = tela de título do jogo**, remontada a partir do layout
  original (`top_play00`) e das texturas da ISO: o fundo rachado, o "5" em
  chamas e o logo BIOHAZARD.
- **Visual dos menus do jogo**: faixa rasgada no topo, abas com as setas e o
  brilho vermelho da seleção, listas como as da Biblioteca e paleta escura
  com texto cor de osso e vermelho.
- **Fonte do jogo embutida** (máquina de escrever e serifada), convertida da
  textura de letras do jogo. Igual em qualquer PC, sem depender das fontes do
  sistema. As letras que o jogo não tem (ã, õ e os acentos da serifada) foram
  montadas com as peças da própria fonte.
- **Personagens**: retratos originais do Chris e da Sheva em qualidade
  original, sobre os painéis *North America* e *West Africa* da tela de
  Organizar Itens.
- **Inventário com as peças do baú do jogo**: células, cursor de seleção e a
  moldura *ARCHIVE*.
- **Armas de inimigos ligadas aos inimigos**: 20 das 38 mostram a Figura do
  inimigo que as usa. 17 são confirmadas pelas próprias Figuras do jogo (o
  arquivo da Figura traz o modelo do inimigo e a arma que ele segura) e 3 têm
  forte evidência (os sons da arma estão dentro do arquivo do inimigo). As
  outras 18 continuam sem imagem: o jogo não mostra quem as usa.
- **Compilação endurecida**: nenhum `unsafe` no código do editor, checagem de
  estouro de inteiros no código do editor, RELRO completo no Linux e Control
  Flow Guard no Windows.

| | |
|---|---|
| ![Personagens](docs/prints/personagens.jpg) | ![Inventário](docs/prints/inventario.jpg) |
| ![Itens Extras](docs/prints/itens-extras.jpg) | ![História](docs/prints/historia.jpg) |

## Novidades da 1.2.0

- **Ícones originais do jogo** em todo o editor: slots do Chris e da Sheva,
  inventário (uma grade de ícones, como o baú do jogo) e escolha do item pelo
  ícone. São 174 ícones, tirados das texturas do próprio RE5 e das DLCs.
- **Ficha do item** no painel de edição: nome oficial, ID, descrição do jogo e
  marca de DLC.
- **Aba Itens Extras**: os registros da tabela de itens do jogo que não vão
  para o save (armas de inimigos, objetos de fase, armas sem nome das DLCs,
  coletáveis ainda não testados e registros internos), com ficha de cada um.
  Só para consulta.
- **Aba História**: os 12 arquivos da Biblioteca, os 26 documentos das fases e
  os 9 documentos das DLCs, **traduzidos para o português**, com leitor.
  Mostra quais arquivos estão desbloqueados no save aberto.
- **Nomes e descrições em português**, com o nome oficial em inglês entre
  parênteses (tradução própria do projeto).
- **Catálogo dos 407 registros** de item (`dados/re5_itens_db.json`), montado
  a partir da tabela interna do jogo (ITEM_INFO_STRUCT) e dos textos do jogo e
  das DLCs.
- Nos slots do Chris e da Sheva, a lista oferece só o que o jogo aceita no
  slot do personagem (armas, munição, cura e coletes). O inventário continua
  com tudo. Um item que já esteja no save nunca some da lista.

## Novidades da 1.1

- **Saves de PC (Steam)**, além do Xbox 360. Ao abrir, o editor descobre sozinho
  de qual plataforma é o save.
- **Desbloqueios**: roupas do Chris e da Sheva, filtros de tela, munição
  infinita (19 armas), Arquivos da Biblioteca (12) e Figuras (45). No Xbox eles
  ficam 0xB4 bytes antes dos do PC (dinheiro em 0xE0 no Xbox e 0x194 no PC).
  Só os bits de cada lista são alterados; bits desconhecidos ficam como
  estavam.
- **Data do save** editável, nas duas plataformas.
- **Interface nativa em Slint**, sem HTML, CSS, JavaScript nem WebView: no
  Windows não precisa do WebView2.
- **Arrastar e soltar** o save na janela, e **Salvar como…**.

## O que edita

| | Xbox 360 | PC |
|---|:---:|:---:|
| Gold / Money e Exchange Points | ✓ | ✓ |
| Data do save | ✓ | ✓ |
| Roupas, filtros, munição infinita, arquivos, figuras | ✓ | ✓ |
| Slots do Chris e da Sheva, inventário (baú) | ✓ (84 espaços) | ✓ (74 espaços) |
| Perfil (XUID), device ID e gamertag | ✓ | — |
| Nome do save no dashboard | ✓ | — |
| Steam ID | — | ✓ |
| Assinatura do save | ✓ sempre (chave interna ou kv.bin) | não precisa |

## Usar

Baixe na página de **Releases** e descompacte:

```
biohazard5-save-editor           Linux (~16 MB)
BIOHAZARD 5 SAVE EDITOR.exe      Windows 10/11
extrair-kv / extrair-kv.exe      tira o kv.bin da cópia da NAND (opcional)
console/kv.bin                   keyvault do seu console (opcional)
backups/                         cópia do save antes de cada gravação
```

1. **Faça backup do pendrive** antes de tudo (copie a pasta `Content` inteira).
2. Abra o programa, clique em **Escolher arquivo** (ou arraste o save para a
   janela):
   - Xbox 360: o `savedata.bin` do pendrive, em `Content\<perfil>\434307D4\00000001\`;
   - PC: `Steam\userdata\<id>\21690\remote\savedata.bin` (no Linux,
     `~/.steam/steam/userdata/<id>/21690/remote/`). O diálogo já abre nessa
     pasta quando a Steam está instalada.
3. Edite e clique em **Salvar arquivo**. Antes de gravar por cima, uma cópia
   vai para a pasta `backups`; depois de gravar, o programa relê o arquivo e
   confere checksum, hashes e assinatura.
4. Devolva o save para a **mesma pasta** do pendrive. Se trocou o perfil
   (XUID), a pasta é a do perfil novo, mostrada no bloco Perfil.

O `kv.bin` é **opcional**: sem ele o save sai assinado pela chave interna.
Com ele, sai assinado pelo seu console. O `kv.bin` é a identidade do seu
console: **não compartilhe**. Como tirar: [docs/EXTRAIR-KV.md](docs/EXTRAIR-KV.md).

Use por sua conta e risco; jogar online com save editado pode trazer
problemas com a Xbox Live.

## Compilar

```bash
cargo build --release                                            # Linux
cargo xwin build --release --target x86_64-pc-windows-msvc       # Windows
cargo test --release                                             # 36 testes
```

O `extrair-kv` é um pacote à parte, no mesmo repositório, e compila sem a
interface:

```bash
cargo build --release -p extrair-kv                                      # Linux
cargo xwin build --release -p extrair-kv --target x86_64-pc-windows-msvc # Windows
cargo test --release -p extrair-kv
```

No Linux precisa de `libgtk-3-dev` (diálogos de arquivo), `libfontconfig1-dev` e
`libxkbcommon-dev` (teclado da janela):

```bash
sudo apt install libgtk-3-dev libfontconfig1-dev libxkbcommon-dev
```

Para o `.exe` do Windows, a partir do Linux:

```bash
sudo apt install clang lld llvm
rustup target add x86_64-pc-windows-msvc
cargo install --locked cargo-xwin
```

`examples/verifica.rs` lê e edita saves pela linha de comando, para testes.

## Versão GOLD: o keyvault do seu console embutido

Opcional: para o save sair assinado **pelo seu console** sem pasta nenhuma, o
`kv.bin` pode ir embutido no executável. O título passa a ser
**BIOHAZARD 5 SAVE EDITOR GOLD**.

```bash
RE5_KV_EMBUTIDO=~/kv-temp/kv.bin cargo build --release --features kv-embutido --target-dir target-pessoal
```

Nunca compartilhe o `kv.bin`, a CPU key, o dump da NAND nem um executável
GOLD. Depois de compilar, apague `target-pessoal` e a cópia solta do `kv.bin`.
O `.gitignore` já deixa `target-*`, `console/` e `*.bin` fora do repositório.

## Estrutura

```
src/save.rs          backend: lê e grava o save (Xbox 360 e PC), checksum, STFS e assinatura
src/perfil.rs        gamertag lida do pacote de perfil do Xbox
src/sistema/         pastas, chaves (interna, de conta e embutida) e diálogos de arquivo
src/interface/       controlador: o único ponto entre a interface e o backend
src/itens.rs         catálogo dos 407 registros de item (src/dados/itens.json)
src/historia.rs      textos da Biblioteca e documentos (src/dados/historia.json)
dados/               banco completo dos 407 registros (re5_itens_db.json)
ui/ponte.slint       tudo que a interface lê e os callbacks que ela chama
ui/app.slint         janela, menus e abas
ui/telas/            início, save, desbloqueios, personagens, inventário,
                     itens extras, história
ui/componentes/      botões, campos, listas, slots, ícones, diálogos
ui/imagens/          ícones, peças dos menus, retratos, figuras e o elenco do Mercenaries
ui/fontes/           fontes do jogo (BH5 Maquina e BH5 Serifa)
ferramentas/         gerador das fontes, extrair-kv (Rust e Python) e testes
```

## Como foi conferido

- **3.0.0**:
  - 36 testes automáticos, entre eles: LIVE/PIRS, outro jogo e outro tipo
    recusados; metadados e miniaturas; bloco fora da tabela; assinatura com
    enchimento errado; leitura de PNG; nome no dashboard; o fim do checksum
    do Xbox; e a gamertag (HMAC-SHA1 e RC4 pelos vetores oficiais).
  - **No console**: save reassinado com a chave interna carrega; save editado
    com o checksum novo carrega, com os itens no jogo (ver "Checksum do Xbox
    corrigido").
  - Saves sem dados depois de `0x4BC0` saem idênticos byte a byte aos da
    2.1.0 (sem assinatura); só mudam, de propósito, os saves em que a 2.1.0
    gravava o checksum errado.
  - Na tela, com cópias de saves reais: aba Save, renomear e salvar
    (gravado, assinado e conferido), gamertag automática e rolagem da
    História até o fim.
- **2.0.0**: 24 saves de Xbox 360 gravados pela 1.2.0 e pela 2.0.0 com três
  conjuntos de edições: 72 de 72 arquivos idênticos byte a byte.
- **PC**: conferido contra uma implementação independente do formato e num
  save real da Steam: abre com o checksum certo, gravar sem mudanças devolve o
  arquivo idêntico, e o jogo mostrou o dinheiro e os pontos editados.

## Licença

MIT — veja [LICENSE](LICENSE).

A interface usa o [Slint](https://slint.dev) sob a Slint Royalty-free License
(o "Sobre" do programa mostra o selo *Made with Slint*, como a licença pede).

*Resident Evil 5* / *BIOHAZARD 5* © CAPCOM CO., LTD. As imagens, texturas,
fontes, ícones e textos do jogo pertencem à Capcom. Projeto de fã, gratuito e
sem fins lucrativos, sem ligação com a Capcom ou a Microsoft.

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-light.svg)](https://slint.dev)
