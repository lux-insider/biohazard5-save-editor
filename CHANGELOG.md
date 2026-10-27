# Histórico de versões

Todas as mudanças relevantes do BIOHAZARD 5 SAVE EDITOR, da mais nova para a
mais antiga. As versões seguem o [Versionamento Semântico](https://semver.org/lang/pt-BR/).

---

## 1.0.0 — 2026-10-19

Primeira versão.

### Correções depois do lançamento
As correções entram na própria 1.0.0 (o download da release é atualizado).

- **26/10 — Aba Save:** o logo do jogo em alta resolução, tirado da tela de
  título, no lugar da miniatura pequena do save.
- **24/10 — Arte da aba Save:** a faixa do topo mostra a Sheva do
  The Mercenaries com a faca, no lugar da Sheva de capuz vermelho.
- **22/10 — Save corrompido no Xbox 360 depois de ativar a munição infinita
  no jogo:** ao ligar o modo, o jogo grava um valor numa área que o editor
  deixava fora do checksum, e o save editado aparecia como corrompido. No
  Xbox o checksum agora inclui essa área; o save de PC continua com a regra
  dele. README e LEIA-ME explicam, com prints do jogo, como ativar a munição
  infinita antes de começar a campanha.
- **21/10 — Uso de memória:** as abas Personagens e Itens extras abriam de
  novo o atlas de ícones (8 MB) em cada ladrilho, e a memória chegava a
  189 MiB. Agora cada imagem é aberta uma vez só e o editor fica entre 70 e
  80 MiB em todas as abas.

### Saves
- **Xbox 360 e PC (Steam)**, com detecção automática da plataforma. No Xbox,
  só abre saves do Resident Evil 5 (Title ID `434307D4`) em pacotes CON.
- **Gold / Money, Exchange Points e data do save.**
- **Desbloqueios:** roupas do Chris e da Sheva, filtros de tela, munição
  infinita das 19 armas (com o ícone de cada uma), os 12 arquivos da
  Biblioteca e as 45 Figuras. Só os bits de cada lista são alterados.
- **Personagens:** os 9 slots do Chris e da Sheva, oferecendo só o que o jogo
  aceita em cada slot.
- **Inventário (baú):** 84 espaços no Xbox e 74 no PC, em grade de ícones como
  no jogo, com filtro por categoria.
- **Só no Xbox:** perfil (XUID) com a gamertag, Device ID ("Copiar de outro
  save…") e nome do save no dashboard (RENOMEAR, com DESFAZER).
- **Só no PC:** Steam ID. O diálogo de abrir já começa na pasta da Steam, no
  Windows e no Linux (nativo, Flatpak e Snap).

### Assinatura do Xbox 360
- O save sai sempre assinado e pronto para o console. Ordem das chaves:
  1. `console/kv.bin` ao lado do programa (o keyvault do seu console);
  2. a chave interna do programa, sempre disponível.
- Checksum do jogo, hashes SHA-1 do pacote e assinatura recalculados e
  conferidos a cada gravação.

### Consulta
- **Catálogo dos 407 registros de item**, com nome oficial, ID, descrição do
  jogo, marca de DLC e os 174 ícones originais do RE5 e das DLCs.
- **Ficha das armas e granadas** com a ilustração da Biblioteca do jogo (30
  ilustrações).
- **Itens Extras:** armas de inimigos (com a Figura do inimigo que usa cada
  uma), objetos de fase e itens das DLCs, só para consulta.
- **História:** os 12 arquivos da Biblioteca com as ilustrações do jogo, os 26
  documentos das fases e os 9 das DLCs, traduzidos para o português.

### Visual
- Tudo vem dos arquivos do *BIOHAZARD 5* e das DLCs, em PNG sem perda: tela
  de título, título com o BIOHAZARD e o 5 de fogo, faixa e abas dos menus,
  listas no estilo da Biblioteca, retratos do Versus, figuras do Chris e da
  Sheva, caixa de itens e a pedra da tela de Recordes.
- Fontes do jogo embutidas (máquina de escrever e serifada), com os acentos do
  português montados com peças da própria fonte.
- Interface nativa em Slint, sem navegador nem WebView.

### Segurança
- **Gravação segura:** grava numa cópia, lê de volta e confere; só então troca
  pelo original. Backup automático na pasta `backups/` antes de cada gravação.
- Itens que travam o jogo (chaves das fases) não são oferecidos; conteúdo de
  DLC é só para consulta.
- O perfil do Xbox é apenas lido; o editor guarda somente "XUID → gamertag".
- Checagem de estouro de inteiros, RELRO
  completo no Linux e Control Flow Guard no Windows.

### Validação
- 46 testes automáticos no editor e 68 no `extrair-kv`.
- **No console:** saves editados e reassinados com a chave interna carregam
  normalmente no Xbox 360, com os itens no jogo.
- **No PC:** num save real da Steam, os valores editados aparecem no jogo, e
  gravar sem alterações devolve o arquivo idêntico byte a byte.
