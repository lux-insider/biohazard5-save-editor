"""Gera as fontes TTF do BIOHAZARD 5 SAVE EDITOR a partir da textura font00 do jogo.

font00: 2304x1152, células de 36x36, 64 por linha (código MSG2 = célula * 2).
Cada letra: canal alfa ampliado 8x -> limiar -> potrace -> contornos em TrueType.
Letras que faltam no jogo (ã õ Ã Õ e todos os acentos da serifada) são
montadas com as peças da própria fonte (letra base + acento de outra letra).

Uso: python3 gera_fonte.py ../font00.png saida/
"""
import os, re, subprocess, sys, tempfile, unicodedata
from PIL import Image
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.cu2quPen import Cu2QuPen
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.svgLib.path import parse_path
import mapa

CEL = 36          # pixels da célula no jogo
AMP = 8           # ampliação antes de vetorizar
UN = 28           # unidades da fonte por pixel do jogo (célula = 1008)
UPM = 1000
ESPACO = 9        # largura do espaço, em pixels do jogo
MARGEM = 2        # espaço de cada lado da letra, em pixels do jogo


def celula(img, n):
    x, y = n % 64 * CEL, n // 64 * CEL
    return img.crop((x, y, x + CEL, y + CEL)).split()[3]


def caixa(a, limiar=100):
    return a.point(lambda v: 255 if v > limiar else 0).getbbox()


def linhas_com_tinta(a, limiar=100):
    px = a.load()
    return [y for y in range(a.height) if any(px[x, y] > limiar for x in range(a.width))]


# ---------------------------------------------------------------- montagem

def acento(img, ch_acentuado, ch_base):
    """Recorta o acento de uma letra da máquina de escrever: a tinta fora da linha da base."""
    inv = {v: k for k, v in mapa.MAQUINA.items()}
    a, b = celula(img, inv[ch_acentuado]), celula(img, inv[ch_base])
    lb = linhas_com_tinta(b)
    topo, pe = min(lb), max(lb)
    cima = a.crop((0, 0, CEL, topo - 1))       # acentos em cima
    baixo = a.crop((0, pe + 1, CEL, CEL))       # cedilha embaixo
    return cima, baixo, topo, pe


def monta(img, base_cel, cima, baixo, topo_ref, pe_ref, sem_pingo_ate=None):
    """Base (de qualquer estilo) + acento centralizado sobre a tinta da base."""
    b = celula(img, base_cel).copy()
    if sem_pingo_ate is not None:              # i e j: tira o pingo antes do acento
        b.paste(0, (0, 0, CEL, sem_pingo_ate))
    bb = caixa(b)
    if bb is None:
        return b
    cx = (bb[0] + bb[2]) / 2
    lb = linhas_com_tinta(b)
    for parte, em_cima in ((cima, True), (baixo, False)):
        pb = caixa(parte)
        if pb is None:
            continue
        peca = parte.crop(pb)
        x = round(cx - peca.width / 2)
        if em_cima:
            y = min(lb) - 2 - peca.height          # 2 px acima da base
        else:
            y = max(lb) + 1 - 3                     # cedilha encosta no pé
        camada = Image.new('L', b.size, 0)
        camada.paste(peca, (x, y))
        b = _max(b, camada)
    return b


def _max(a, b):
    from PIL import ImageChops
    return ImageChops.lighter(a, b)


# ---------------------------------------------------------------- vetorização

def vetoriza(a):
    """Canal alfa 36x36 -> lista de caminhos SVG (coordenadas ampliadas)."""
    grande = a.resize((CEL * AMP, CEL * AMP), Image.BICUBIC)
    pbm = grande.point(lambda v: 0 if v > 110 else 255).convert('1')
    with tempfile.TemporaryDirectory() as t:
        entrada, saida = os.path.join(t, 'g.pbm'), os.path.join(t, 'g.svg')
        pbm.save(entrada)
        subprocess.run(['potrace', '-s', '--flat', '-t', '4', '-a', '1.0', '-O', '0.4',
                        '-u', '1', '-o', saida, entrada], check=True)
        svg = open(saida).read()
    m = re.search(r'<g transform="([^"]+)"', svg)
    return re.findall(r' d="([^"]+)"', svg), m.group(1) if m else ''


def glifo(a, base_px):
    """Retorna (glifo TrueType, largura) com a linha de base em base_px (pixel do jogo)."""
    bb = caixa(a)
    pen = TTGlyphPen(None)
    if bb is None:
        return pen.glyph(), ESPACO * UN
    caminhos, transf = vetoriza(a)
    # potrace: translate(0,H) scale(0.1,-0.1) com -u 1 -> coordenadas em pixels ampliados, y para cima
    tr = re.findall(r'translate\(([-\d.]+),([-\d.]+)\)', transf)
    sc = re.findall(r'scale\(([-\d.]+),([-\d.]+)\)', transf)
    tx, ty = map(float, tr[0]) if tr else (0.0, 0.0)
    sx, sy = map(float, sc[0]) if sc else (1.0, 1.0)
    k = UN / AMP
    dx = (MARGEM - bb[0]) * UN
    # ponto svg (px, py) -> ampliado (tx + sx*px, ty + sy*py) com y para baixo
    # fonte: x = ampliado_x*k + dx ; y = (base_px*AMP - ampliado_y)*k
    matriz = (sx * k, 0, 0, -sy * k, tx * k + dx, (base_px * AMP - ty) * k)
    cu = Cu2QuPen(TransformPen(pen, matriz), max_err=1.0, reverse_direction=True)
    for d in caminhos:
        parse_path(d, cu)
    largura = (bb[2] - bb[0] + 2 * MARGEM) * UN
    return pen.glyph(), largura


# ---------------------------------------------------------------- fonte

def nome_glifo(ch):
    return 'uni%04X' % ord(ch)


def gera(img, estilo, tabela, saida, familia):
    inv = {v: k for k, v in mapa.MAQUINA.items()}
    celulas = dict((ch, ('cel', n)) for n, ch in tabela.items())
    for n, ch in mapa.SIMBOLOS.items():
        celulas.setdefault(ch, ('cel', n))
    for ch in '/<>=_|@$[]"\'~':   # o que a serifada não tem vem da máquina de escrever
        if ch in inv:
            celulas.setdefault(ch, ('cel', inv[ch]))

    # linha de base: pé do 'H' do estilo
    h = celula(img, [n for n, c in tabela.items() if c == 'H'][0])
    base = max(linhas_com_tinta(h)) + 1
    alt_maiusc = base - min(linhas_com_tinta(h))

    # letras acentuadas que faltam: base do estilo + acento da máquina de escrever
    fontes_acento = {}
    for ch in 'áàâäãéèêëíìîïóòôöõúùûüçñÁÀÂÄÃÉÈÊËÍÌÎÏÓÒÔÖÕÚÙÛÜÇÑ':
        if ch in celulas:
            continue
        dec = unicodedata.normalize('NFD', ch)
        letra, marca = dec[0], dec[1:]
        if letra not in celulas or celulas[letra][0] != 'cel':
            continue
        # acento emprestado: mesma marca na máquina de escrever (tilde vem do ñ / Ñ)
        doador = {'̃': 'ñ' if letra.islower() else 'Ñ'}.get(marca)
        if doador is None:
            doador = unicodedata.normalize('NFC', ('a' if letra.islower() else 'A') + marca)
            if marca == '̧':
                doador = 'ç' if letra.islower() else 'Ç'
            if doador not in inv:
                doador = unicodedata.normalize('NFC', ('e' if letra.islower() else 'E') + marca)
        base_doador = unicodedata.normalize('NFD', doador)[0]
        if doador not in inv or base_doador not in inv:
            continue
        cima, baixo, _, _ = acento(img, doador, base_doador)
        pingo = None
        if letra in 'ij':
            n_cel = [k for k, c in tabela.items() if c == 'n'][0]
            pingo = min(linhas_com_tinta(celula(img, n_cel))) - 1
        fontes_acento[ch] = monta(img, celulas[letra][1], cima, baixo, 0, 0, pingo)

    ordem = ['.notdef', 'space'] + [nome_glifo(c) for c in sorted(set(celulas) | set(fontes_acento)) if c != ' ']
    glifos, metricas, cmap = {}, {}, {0x20: 'space'}
    g, w = glifo(Image.new('L', (CEL, CEL), 0), base)
    glifos['.notdef'], metricas['.notdef'] = g, (w, 0)
    glifos['space'], metricas['space'] = g, (ESPACO * UN, 0)
    for ch in sorted(set(celulas) | set(fontes_acento)):
        if ch == ' ':
            continue
        a = fontes_acento[ch] if ch in fontes_acento else celula(img, celulas[ch][1])
        g, w = glifo(a, base)
        n = nome_glifo(ch)
        glifos[n] = g
        metricas[n] = (w, 0)
        cmap[ord(ch)] = n
    # hífen do Unicode também aponta para o '-'
    if ord('-') in cmap:
        cmap[0x2010] = cmap[0x2011] = cmap[ord('-')]
    for novo, velho in mapa.APELIDOS.items():
        if ord(velho) in cmap:
            cmap.setdefault(ord(novo), cmap[ord(velho)])

    fb = FontBuilder(UPM, isTTF=True)
    fb.setupGlyphOrder(ordem)
    fb.setupCharacterMap(cmap)
    fb.setupGlyf(glifos)
    glyf = fb.font['glyf']
    lsb = {}
    for n in ordem:
        glyf[n].recalcBounds(glyf)
        lsb[n] = getattr(glyf[n], 'xMin', 0)
    fb.setupHorizontalMetrics({n: (metricas[n][0], lsb[n]) for n in ordem})
    asc = base * UN
    desc = (CEL - base) * UN
    fb.setupHorizontalHeader(ascent=asc, descent=-desc)
    fb.setupNameTable({
        'familyName': familia,
        'styleName': 'Regular',
        'uniqueFontIdentifier': familia + ' Regular',
        'fullName': familia,
        'psName': familia.replace(' ', ''),
        'version': 'Version 1.000',
        'copyright': 'Desenho das letras: BIOHAZARD 5 / Resident Evil 5 (c) CAPCOM CO., LTD. '
                     'Convertido da textura do jogo para uso no BIOHAZARD 5 SAVE EDITOR (projeto de fã, sem fins lucrativos).',
    })
    fb.setupOS2(sTypoAscender=asc, sTypoDescender=-desc, sTypoLineGap=0,
                usWinAscent=asc, usWinDescent=desc, sxHeight=0, sCapHeight=alt_maiusc * UN,
                fsType=0)
    fb.setupPost()
    fb.save(saida)
    return len(cmap), sorted(fontes_acento)


if __name__ == '__main__':
    img = Image.open(sys.argv[1]).convert('RGBA')
    os.makedirs(sys.argv[2], exist_ok=True)
    for estilo, tabela, arq, fam in [
        ('maquina', mapa.MAQUINA, 'BH5-Maquina.ttf', 'BH5 Maquina'),
        ('serifa', mapa.SERIFA, 'BH5-Serifa.ttf', 'BH5 Serifa'),
    ]:
        n, montadas = gera(img, estilo, tabela, os.path.join(sys.argv[2], arq), fam)
        print(arq, n, 'caracteres; montadas:', ''.join(montadas))
