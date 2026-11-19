# Histórico de versões

Todas as mudanças relevantes do BIOHAZARD 5 SAVE EDITOR, da mais nova para a
mais antiga. As versões seguem o [Versionamento Semântico](https://semver.org/lang/pt-BR/).

---

## 1.0.0 — 2026-10-19

Primeira versão.

### Correções depois do lançamento
As correções entram na própria 1.0.0 (o download da release é atualizado).

- **19/11 — Abas Figuras e História:** saem da barra de abas. Os
  desbloqueios delas continuam na aba Desbloqueios (Figuras, Figuras extras
  e os 12 arquivos da Biblioteca), e o jogo mostra tudo como antes. As
  telas continuam no código, só não aparecem.
- **19/11 — Desbloqueios sem rolagem:** cada lista escolhe o número de
  colunas e, se precisar, diminui a altura das linhas para caber inteira.
  As 46 Figuras e as 19 armas da munição infinita aparecem de uma vez, sem
  barra de rolagem, no tamanho normal da janela.
- **19/11 — Ícone:** o 5 de fogo da tela de título agora vem com o fogo em
  volta, recortado com a máscara que o próprio jogo usa nessa textura.
- **19/11 — Ícone e nome no .exe:** o .exe do Windows publicado desde 18/11
  saía sem o ícone e sem o nome do programa nas propriedades do arquivo;
  volta a ter os dois.
- **19/11 — Fundo da aba Save:** sai a arte grande do Chris. À esquerda
  entram, deitados, o Chris (crachás e pistola) e a Sheva (crachá e colar)
  da mesa do menu do jogo e, nos cantos vazios, redes orgânicas dos efeitos
  do jogo, junto com as células e veias da tela de título. Nada passa por
  cima dos textos, da linha das abas nem do banner.
- **19/11 — Células e veias nítidas:** as células e veias do fundo da aba Save
  também ganham cópias já redimensionadas com filtro e não ficam mais
  serrilhadas.
- **18/11 — Imagens nítidas:** os ícones, as ilustrações das armas, as
  imagens da Biblioteca, os retratos e as figuras do Chris e da Sheva e o
  fundo de pedra não ficam mais serrilhados nem em blocos. A interface
  reduzia e ampliava as imagens repetindo ou pulando pixels; agora cada uma
  tem cópias já redimensionadas com filtro, a partir da mesma arte do jogo,
  nos tamanhos em que aparece, também com a escala do sistema em 125% e 150%.
  O executável ficou cerca de 8 MB maior.
- **18/11 — Fonte do jogo nos textos:** os textos das abas voltam à
  máquina de escrever do jogo, agora tirada da versão de PC, num tamanho
  normal e com a sombra escura que o jogo põe nas letras. A Noto Sans Mono
  fica só como reserva para os poucos caracteres que o jogo não tem.
  Títulos, botões e a tela inicial não mudaram.
- **18/11 — Ilustração das armas:** no painel do slot, a ilustração da
  Biblioteca fica à esquerda do nome, sem o retângulo preto, e sobra mais
  altura para a lista de escolha.
- **18/11 — Samurai Edge do Barry:** a 0x0112, arma do Barry no The
  Mercenaries Reunion, entra no baú e nos slots do Chris e da Sheva, com o
  ícone dela (a pistola preta com o emblema dos S.T.A.R.S.). Testada no
  jogo da Steam: atira, recarrega e continua no save depois que o jogo
  grava. Na campanha ela fica no slot sem figura, porque o ícone está numa
  textura que só o Mercenaries Reunion e as DLCs carregam.
- **18/11 — Samurai Edge do Wesker:** passa a mostrar o ícone que o jogo usa
  para ela (a pistola com lanterna) no lugar da silhueta da loja.
- **18/11 — Ficha das pistolas:** a Px4 e as duas Samurai Edge mostram no
  painel a ficha lida no jogo: potência, capacidade, recarga, crítico,
  perfuração e mira. O crítico da Px4 (×7) dá 87,5% de chance num tiro na
  cabeça de um Majini; as outras duas, 12,5%. O painel avisa também que as
  três vendem por ₦0.
- **18/11 — Abas Itens Extras e Inimigos:** saíram. Eram só de consulta e
  não mudavam nada no save; o editor fica com as abas que editam o save e a
  História. O executável ficou cerca de 0,5 MB menor.
- **13/11 — Nomes e moldura:** os nomes com o inglês não têm mais
  parênteses dentro de parênteses: "Ovo podre (Egg, Rotten)", "Erva V+Vm
  (Herb, G+R)". Nos quadrados do baú, dos tesouros, da escolha do item e dos
  itens extras aparece só o nome em português, sem reticências; o nome
  completo fica nos slots dos personagens e no painel do item. O spray usa o
  nome do jogo, First aid spray. Nos slots dos personagens, a linha de cima
  (slot, quantidade, "alterado" e classe) não passa mais da borda do cartão.
  A moldura do item escolhido fica na borda do quadrado, com os cantos retos
  e sem brilho por cima do número, da quantidade e do ícone.
- **13/11 — Fonte dos textos:** os textos das abas passam a usar a Noto Sans
  Mono em negrito, bem mais legível que a máquina de escrever do jogo em
  letra pequena, e ficaram um pouco menores para a tela não ficar carregada.
  Títulos e botões continuam na serifada do jogo, e a tela inicial (antes de
  abrir um save) não mudou.
- **11/11 — Leitura das telas:** os nomes dos itens não são mais cortados:
  até três linhas no baú e na escolha do item, e até duas nos slots dos
  personagens (a quantidade passou para a linha do número do slot). No baú,
  a quantidade tem um fundo escuro para ser lida sobre ícones claros; os textos de apoio ficaram mais claros; e uma
  arma descarregada, como o lança-granadas, não mostra mais "× 0". Nas
  melhorias, as estrelas (Crítico, Perfuração, Alcance) ficaram maiores e o
  nível aparece em número à direita.
- **11/11 — Licenças:** o programa passa a vir com o arquivo
  `AVISOS-DE-TERCEIROS.txt`, com a lista das bibliotecas de código aberto
  usadas e os textos de licença delas; a janela Sobre cita o arquivo. A
  licença de uso do programa (freeware) aponta para ele.
- **11/11 — Restaurar backup:** no menu Arquivo, "Restaurar backup…" abre
  a pasta das cópias do save aberto; a escolhida toma o lugar do save, depois
  de uma confirmação. O save que estava lá também é guardado antes, e um
  arquivo que não é save do jogo é recusado. Os backups antigos servem.
- **11/11 — Arquivo alterado por fora:** se outro programa (ou o jogo)
  gravar no save depois que ele foi aberto no editor, salvar não escreve
  mais por cima sem avisar: o editor pergunta antes. Confirmando, grava o
  que está na tela e guarda nos backups o arquivo como estava no disco.
- **11/11 — Backups:** cada perfil do Xbox e cada conta do PC tem agora a
  sua pasta dentro de `backups/`, com a data e a hora no nome do arquivo, e
  ficam as 10 cópias mais novas de cada save (eram 20, sem separar por
  dono). Os backups feitos antes continuam onde estão e nunca são apagados.
- **11/11 — Abrir:** um arquivo com mais de 4 MB é recusado em vez de ser
  lido inteiro para a memória. Sem save do RE5 na Steam, o diálogo de abrir
  começa na pasta do save do Xbox em Documentos, ou na pasta pessoal.
- **11/11 — Keyvault:** um `console/kv.bin` cujo certificado não é o da
  chave é recusado ("keyvault inválido") em vez de assinar um save que o
  console não aceitaria.
- **10/11 — Arma equipada:** no painel de um slot do Chris ou da Sheva, o
  botão EQUIPAR põe a marca E na arma (a arma com que o personagem começa o
  capítulo), e o cartão do slot mostra o E. Testado no jogo da Steam.
- **10/11 — Loja do jogo:** na aba Desbloqueios, o botão LIBERAR ARMAS NA LOJA
  põe as 18 armas à venda. Só liga os bits das armas e preserva os outros.
- **10/11 — Backups:** ficam os 20 mais recentes de cada save; duas gravações
  no mesmo segundo não apagam mais o backup anterior.
- **10/11 — Correções:** a aba Figuras volta às Figuras ao abrir um save de
  Xbox com as extras na tela; um nome de arquivo antigo (Latin-1, no Linux)
  não fecha mais o programa; um pacote do Xbox corrompido dá erro em vez de
  usar memória sem limite; o arquivo 1 da Biblioteca tem o mesmo nome nas
  abas Desbloqueios e História.
- **10/11 — Removido:** a ferramenta `extrair-kv` saiu do projeto. O `kv.bin`
  continua opcional, na pasta `console/`.
- **04/11 — Windows:** o `.exe` fechava ao abrir no Windows, por causa de uma proteção da compilação (Control Flow Guard). Sem ela, abre normalmente. Testado no Windows 10.
- **03/11 — Baú com o tamanho certo (Xbox 360 e PC):** o baú do jogo tem 54
  itens e 50 tesouros, mas o editor mostrava 84 espaços no Xbox e 74 no PC como
  se fossem todos iguais. Os tesouros apareciam pela metade e os últimos
  espaços eram dados da partida, como a dificuldade e a fase. Agora o editor
  separa os 54 itens dos 50 tesouros, com uma aba própria e o botão "Colocar
  todos", e não grava mais nos dados da partida. A aba Inventário não oferece
  mais tesouro como item.
- **04/11 — Melhorias das armas:** nos slots e no inventário, o botão Melhorar
  mostra Potência, Recarregamento, Capacidade e as estrelas de cada arma, só até
  o limite dela. Trocar ou esvaziar um item agora zera os níveis, para a arma
  nova não herdar os da anterior.
- **04/11 — Escolhas e rolagem:** as listas suspensas que cobriam os itens
  viraram botões sempre à vista; a barra de rolagem ficou fina, com a cor do
  jogo e numa faixa própria, em todas as abas; o painel de edição ficou mais
  largo e mostra mais itens. O filtro do baú não oferece mais Tesouro, Chave e
  Arquivo.
- **03/11 — Quantidade máxima de 32.767:** no baú e nos slots do Chris e da
  Sheva. Testado no jogo da Steam: acima disso, as quantidades aparecem
  negativas ou erradas. A aba Inventário agora avisa que quantidades altas
  voltam ao padrão do jogo ao mover o item para os slots dos personagens.
- **29/10 — Abas novas e visual:** aba **Figuras** com a máquina de cápsulas
  e as bolinhas do jogo (46 + 10 extras no PC); aba **Inimigos** com as fotos
  e os nomes da tela de Recordes (inimigos, animais e personagens); a aba Save
  ganhou Chris e Sheva da arte do The Mercenaries, células e veias da tela de
  título ao fundo, e a faixa e os painéis ficaram translúcidos; a linha de
  baixo da barra de abas ficou mais marcada; Itens extras sem o grupo das
  armas de inimigos; a aba História sem a imagem do arquivo "A História de
  RESIDENT EVIL"; ícone novo: o 5 de fogo da tela de título, sem fundo.
- **28/10 — Figuras:** a lista ganhou a Nº 46, Wesker (raro), e o save de PC
  ganhou a lista das 10 Figuras extras (menu Extra Figures), conferidas no
  jogo da Steam.
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
  Biblioteca e as 46 Figuras. Só os bits de cada lista são alterados.
- **Personagens:** os 9 slots do Chris e da Sheva, oferecendo só o que o jogo
  aceita em cada slot.
- **Inventário (baú):** 54 itens e 50 tesouros, em grade de ícones como
  no jogo, com filtro por categoria.
- **Só no Xbox:** perfil (XUID) com a gamertag, Device ID ("Copiar de outro
  save…") e nome do save no dashboard (RENOMEAR, com DESFAZER).
- **Só no PC:** Steam ID. O diálogo de abrir já começa na pasta da Steam, no
  Windows e no Linux (nativo, Flatpak e Snap).

### Assinatura do Xbox 360
- O save sai sempre assinado e pronto para o console. Ordem das chaves:
  1. `console/kv.bin` ao lado do programa (o keyvault do seu console);
  2. o kv.bin embutido numa compilação pessoal GOLD (`--features kv-embutido`);
  3. a chave interna do programa, sempre disponível.
- Checksum do jogo, hashes SHA-1 do pacote e assinatura recalculados e
  conferidos a cada gravação.

### Consulta
- **Catálogo dos 407 registros de item**, com nome oficial, ID, descrição do
  jogo, marca de DLC e os 174 ícones originais do RE5 e das DLCs.
- **Ficha das armas e granadas** com a ilustração da Biblioteca do jogo (30
  ilustrações).
- **Itens Extras:** objetos de fase, itens das DLCs, não testados e internos,
  só para consulta.
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
- Nenhum `unsafe` no código do editor, checagem de estouro de inteiros e RELRO
  completo no Linux.

### Validação
- 58 testes automáticos no editor e 68 no `extrair-kv`.
- **No console:** saves editados e reassinados com a chave interna carregam
  normalmente no Xbox 360, com os itens no jogo.
- **No PC:** num save real da Steam, os valores editados aparecem no jogo, e
  gravar sem alterações devolve o arquivo idêntico byte a byte.
