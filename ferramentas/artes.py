"""Gera as imagens da interface a partir das artes extraídas da ISO do RE5 e do
pacote de DLC (pasta "RE5 - Integração Visual"), sempre em PNG, sem perda.

Uso: python3 ferramentas/artes.py "<pasta RE5 - Integração Visual>"
(precisa de Pillow)

As texturas do jogo já vêm comprimidas pelo próprio jogo (DXT, em blocos de
4x4); por isso o editor não acrescenta outra compressão com perda (JPEG) nem
reduz os ícones: cada ícone fica no tamanho em que está na textura do jogo.

O que este script gera:
  icones.png         atlas dos ícones de item: 16 colunas, células de 102 px
                     (o tamanho de cada ícone na textura cp05/cp26 do jogo)
  jogo/titulo.png    tela de título remontada (top_play00, ver abaixo)
  jogo/painel_*.png  painéis do Chris (itembox0a) e da Sheva (itembox06)
  jogo/caixa_itens.png  foto da caixa de itens (itembox01)
  jogo/pedra.png     a pedra da tela de Recordes (tm151), escurecida
  biblioteca/*.png   as ilustrações dos 12 arquivos da Biblioteca
  retratos/*.png     retratos dos cartões da aba Personagens (Versus): o Chris
                     de colete preto (versus_17) e a Sheva (versus_20,
                     espelhada para olhar para o centro)
  jogo/figura_*.png  Chris e Sheva da faixa embaixo dos slots (tm152, DLC)
  armas.png          atlas das ilustrações das armas na Biblioteca (quadros de
                     320x240, 6 colunas); grava em src/dados/itens.json o
                     campo "ilustracao" (posição no atlas, -1 = sem ilustração)
"""
import json, os, sys
from PIL import Image, ImageChops, ImageEnhance

RAIZ = sys.argv[1]
ARTES = os.path.join(RAIZ, 'artes')
REPO = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')
UI = os.path.join(REPO, 'ui', 'imagens')


def abre(rel):
    return Image.open(os.path.join(ARTES, rel)).convert('RGBA')


def salva(img, rel):
    caminho = os.path.join(UI, rel)
    os.makedirs(os.path.dirname(caminho), exist_ok=True)
    img.save(caminho, optimize=True)
    print(rel, img.size)


# ---------------------------------------------------------------- ícones
# Cada item traz, em dados/re5_itens_db.json, o recorte do seu ícone na
# textura do jogo (células de 102 px). As silhuetas da loja (itembox03) já
# estão no tamanho original no atlas anterior e são copiadas como estão.
CELULA, COLUNAS = 102, 16
TEXTURAS = {
    'cp05': 'dlc/Game2Resource_e/id_jpn_cockpit_cp05_360_NOMIP_BM.png',
    'cp26': 'dlc/Game2Resource_e/id_jpn_cockpit_cp26_360_NOMIP_BM.png',
}
# Ícones que o atlas mostra centralizados na célula (a peça fica num canto
# do recorte do jogo): as estrelas de pontos e os fragmentos da serpente.
CENTRALIZADOS = {92, 93, 94, 162, 163, 164, 165}
SILHUETAS_ANTIGAS = os.path.join(REPO, 'ferramentas', 'silhuetas_loja.png')

itens_p = os.path.join(REPO, 'src', 'dados', 'itens.json')
itens = json.load(open(itens_p, encoding='utf-8'))
db = {int(i['id'], 16): i for i in json.load(open(os.path.join(REPO, 'dados', 're5_itens_db.json'), encoding='utf-8'))['itens']}
texturas = {k: abre(v) for k, v in TEXTURAS.items()}
silhuetas = Image.open(SILHUETAS_ANTIGAS).convert('RGBA')

origem = {}
for it in itens['itens']:
    k = it['icone']
    if k < 0 or k in origem:
        continue
    ic = (db.get(it['id']) or {}).get('icone')
    if ic:
        origem[k] = (ic['textura'], tuple(ic['rect']))
total = 1 + max(it['icone'] for it in itens['itens'])
linhas = (total + COLUNAS - 1) // COLUNAS
atlas = Image.new('RGBA', (COLUNAS * CELULA, linhas * CELULA))
n_silhueta = 0
for k in range(total):
    x, y = k % COLUNAS * CELULA, k // COLUNAS * CELULA
    if k in origem:
        t, r = origem[k]
        peca = texturas[t].crop(r)
        if k in CENTRALIZADOS:
            peca = peca.crop(peca.getchannel('A').point(lambda v: 255 if v > 40 else 0).getbbox())
            x += (CELULA - peca.width) // 2
            y += (CELULA - peca.height) // 2
        atlas.alpha_composite(peca, (x, y))
    else:
        # silhueta da loja: copiada do atlas anterior (72 px), centralizada
        s = silhuetas.crop((n_silhueta * 72, 0, n_silhueta * 72 + 72, 72))
        atlas.alpha_composite(s, (x + (CELULA - 72) // 2, y + (CELULA - 72) // 2))
        n_silhueta += 1
salva(atlas, 'icones.png')
assert itens['celula'] == CELULA and itens['colunas_atlas'] == COLUNAS, 'atualize "celula" em src/dados/itens.json'

# ---------------------------------------------------------------- artes
# Tela de título remontada pelo layout do jogo (render.py --vis), sem o logo
# RESIDENT EVIL (re_fo_l), sem os "PRESS START" (start*) e sem os fragmentos
# de pedra da abertura (ti_*), que no jogo ficam sobrepostos só durante a animação.
salva(Image.open(os.path.join(RAIZ, 'telas remontadas', 'id_jpn_title_top_play00_editor.png')).convert('RGB'), 'jogo/titulo.png')
salva(abre('iso/ShopResource/id_jpn_itembox_itembox0a_360_NOMIP_BM.png').convert('RGB'), 'jogo/painel_chris.png')
salva(abre('iso/ShopResource/id_jpn_itembox_itembox06_360_NOMIP_BM.png').convert('RGB'), 'jogo/painel_sheva.png')
salva(abre('iso/ShopResource/id_jpn_itembox_itembox01_360_NOMIP_BM.png').convert('RGB'), 'jogo/caixa_itens.png')

# Pedra: as duas células vazias da tela de Recordes, lado a lado, escurecidas.
tm151 = abre('iso/RecordResource/id_jpn_menu_tm151_360_NOMIP_BM.png').convert('RGB')
pedra = Image.new('RGB', (640, 240))
pedra.paste(tm151.crop((320, 240, 640, 480)), (0, 0))
pedra.paste(tm151.crop((640, 240, 960, 480)), (320, 0))
salva(ImageEnhance.Brightness(pedra).enhance(0.55), 'jogo/pedra.png')

# ---------------------------------------------------------------- Biblioteca
VIEWER = 'iso/ViewerResource/id_jpn_menu_tm%d_360_NOMIP_BM.png'
FUNDO = (0x15, 0x12, 0x0E, 255)
t120, t121, t122 = (abre(VIEWER % n) for n in (120, 121, 122))


def chapa(im):
    f = Image.new('RGBA', im.size, FUNDO)
    f.alpha_composite(im)
    return f.convert('RGB')


def recorta(im, caixa, margem=6, limiar=12):
    """Recorta a caixa e aperta no que tem tinta (nem preto nem transparente)."""
    r = im.crop(caixa)
    a = r.getchannel('A').point(lambda v: 255 if v > limiar else 0)
    l = r.convert('L').point(lambda v: 255 if v > limiar else 0)
    x0, y0, x1, y1 = ImageChops.multiply(a, l).getbbox()
    return r.crop((max(0, x0 - margem), max(0, y0 - margem), min(r.width, x1 + margem), min(r.height, y1 + margem)))


bib = {
    'saga': recorta(t120, (0, 0, 600, 380)),
    'bsaa': recorta(t120, (700, 0, 1024, 390)),
    'ndipaya': recorta(t121, (0, 0, 640, 320)),
    'u8': recorta(t121, (0, 320, 640, 640)),
    'majini': recorta(t122, (0, 0, 740, 420)),
    'tricell': recorta(t122, (780, 0, 1024, 680)),
}
# o notebook da Tricell, com a ficha do Irving, da Excella ou do Wesker na tela
notebook = t122.crop((20, 420, 540, 820))
bib['irving'] = recorta(notebook, (0, 0, notebook.width, notebook.height))
for nome, tela in (('excella', (0, 832, 272, 1022)), ('wesker', (272, 832, 544, 1022))):
    n = notebook.copy()
    n.alpha_composite(t122.crop(tela), (49, 45))
    bib[nome] = recorta(n, (0, 0, n.width, n.height))
# cartões de identificação: inteiros
for nome, caixa in (('chris', (0, 404, 336, 1024)), ('sheva', (336, 404, 704, 1024)), ('jill', (704, 404, 1024, 1024))):
    bib[nome] = t120.crop(caixa)
for nome, im in bib.items():
    salva(chapa(im), 'biblioteca/%s.png' % nome)

# ---------------------------------------------------------------- Personagens
# Retratos dos cartões (Versus, 736x736, inteiros).
VERSUS = 'iso/VersusResource/id_jpn_versus_versus_%d_360_NOMIP_BM.png'
salva(abre(VERSUS % 17), 'retratos/chris.png')
salva(abre(VERSUS % 20).transpose(Image.Transpose.FLIP_LEFT_RIGHT), 'retratos/sheva.png')
# Faixa embaixo dos slots: a Sheva e o Chris da textura tm152 (Extra1, DLC).
# As figuras ocupam as linhas 173 a 670; em cima fica outra peça (borrada).
tm152 = abre('dlc/Extra1Resource/id_jpn_menu_tm152_360_NOMIP_BM.png')
for nome, x0 in (('sheva', 0), ('chris', 512)):
    peca = tm152.crop((x0, 173, x0 + 512, 671))
    x_a, _, x_b, _ = peca.getchannel('A').point(lambda v: 255 if v > 8 else 0).getbbox()
    salva(peca.crop((x_a, 0, x_b, peca.height)), 'jogo/figura_%s.png' % nome)

# ---------------------------------------------------------------- armas
# Ilustrações das armas na Biblioteca (RecordResource tm100/103/104/105):
# quadros de 320x240 em 3 colunas. Cada entrada: textura, quadro e os IDs que
# usam a ilustração. Lança-chamas, RPG-7 NVS e os lança-granadas de munição
# especial não têm ilustração no jogo e ficam com o ícone.
RECORD = 'iso/RecordResource/id_jpn_menu_tm%d_360_NOMIP_BM.png'
QUADRO_L, QUADRO_A, COLUNAS_ARMAS = 320, 240, 6
ILUSTRACOES = [
    (103, 0, [0x102]),   # M92F
    (103, 1, [0x110]),   # H&K P8
    (103, 2, [0x111]),   # SIG P226
    (103, 3, [0x11F]),   # Px4
    (103, 4, [0x11E]),   # M93R
    (103, 5, [0x104]),   # Ithaca M37
    (103, 6, [0x116]),   # M3
    (103, 7, [0x117]),   # Jail Breaker
    (103, 8, [0x119]),   # Hydra
    (103, 9, [0x105]),   # S75
    (103, 10, [0x120]),  # Dragunov SVD
    (103, 11, [0x11C]),  # H&K PSG-1
    (104, 0, [0x103]),   # VZ61
    (104, 1, [0x113]),   # H&K MP5
    (104, 2, [0x109]),   # SIG 556
    (104, 3, [0x11D]),   # AK-74
    (104, 4, [0x115]),   # Metralhadora giratória
    (104, 5, [0x10B]),   # S&W M29
    (104, 6, [0x11A]),   # L. Hawk
    (104, 7, [0x11B]),   # S&W M500
    (104, 8, [0x10C]),   # Lança-granadas
    (104, 9, [0x10D]),   # Lança-foguetes
    (104, 10, [0x122]),  # Bastão de choque
    (104, 11, [0x10F]),  # Arco longo
    (105, 0, [0x101, 0x10E, 0x123, 0x124]),  # Faca (Chris, Sheva, Wesker, Jill)
    (105, 1, [0x106]),   # Granada de mão
    (105, 2, [0x107]),   # Granada incendiária
    (105, 3, [0x108]),   # Granada de luz
    (105, 4, [0x10A]),   # Bomba de proximidade
    (100, 8, [0x129, 0x118]),                 # Samurai Edge
]
texturas_rec = {}
linhas_armas = (len(ILUSTRACOES) + COLUNAS_ARMAS - 1) // COLUNAS_ARMAS
armas = Image.new('RGBA', (COLUNAS_ARMAS * QUADRO_L, linhas_armas * QUADRO_A))
posicao = {}
for k, (tm, q, ids) in enumerate(ILUSTRACOES):
    t = texturas_rec.setdefault(tm, abre(RECORD % tm))
    x, y = q % 3 * QUADRO_L, q // 3 * QUADRO_A
    armas.paste(t.crop((x, y, x + QUADRO_L, y + QUADRO_A)), (k % COLUNAS_ARMAS * QUADRO_L, k // COLUNAS_ARMAS * QUADRO_A))
    for i in ids:
        posicao[i] = k
salva(armas, 'armas.png')
for it in itens['itens']:
    it['ilustracao'] = posicao.pop(it['id'], -1)
assert not posicao, 'IDs fora de src/dados/itens.json: %s' % [hex(i) for i in posicao]
with open(itens_p, 'w', encoding='utf-8') as f:
    f.write(json.dumps(itens, ensure_ascii=False, separators=(',', ':')))
print('ok')
