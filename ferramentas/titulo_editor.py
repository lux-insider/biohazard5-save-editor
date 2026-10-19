"""Monta o título do editor (ui/imagens/jogo/titulo_editor.png) com as peças da
tela de título do RE5: o BIOHAZARD vermelho e o 5 de fogo (TitleResource,
title_backpic03) e SAVE EDITOR na fonte BH5 Serifa.

Uso: python titulo_editor.py <title_backpic03.png> <BH5-Serifa.ttf> <saida.png> <escala>
"""
import sys
from PIL import Image, ImageDraw, ImageFont, ImageFilter
src, fonte, saida, escala = sys.argv[1], sys.argv[2], sys.argv[3], float(sys.argv[4])
t = Image.open(src).convert('RGBA')
def peca(box):
    p = t.crop(box); return p.crop(p.split()[3].point(lambda v: 255 if v > 20 else 0).getbbox())
vermelho = peca((0, 800, 690, 885))
brilho = peca((0, 885, 690, 975))
cinco = peca((800, 110, 1024, 625))
H = int(round(64 * escala))            # altura total
hb = int(round(46 * escala))           # altura do BIOHAZARD
def alt(im, h): return im.resize((max(1, round(im.width * h / im.height)), h), Image.LANCZOS)
v = alt(vermelho, hb); g = alt(brilho, hb); c = alt(cinco, H)
f = ImageFont.truetype(fonte, int(round(21 * escala)))
txt = 'SAVE EDITOR'; esp = int(round(5 * escala))
larg_txt = sum(f.getbbox(ch)[2] for ch in txt) + esp * (len(txt) - 1)
W = v.width + int(6 * escala) + c.width + int(18 * escala) + larg_txt + int(4 * escala)
img = Image.new('RGBA', (W, H), (0, 0, 0, 0))
yb = (H - hb) // 2 + int(2 * escala)
# brilho de fogo atrás das letras: a própria forma, borrada, em laranja
forma = Image.new('RGBA', img.size, (0, 0, 0, 0)); forma.paste(v, (0, yb))
a = forma.split()[3].filter(ImageFilter.GaussianBlur(5 * escala)).point(lambda x: int(x * 0.55))
fogo = Image.new('RGBA', img.size, (255, 110, 20, 0)); fogo.putalpha(a)
img.alpha_composite(fogo); img.alpha_composite(v, (0, yb))
xc = v.width + int(6 * escala)
img.alpha_composite(c, (xc, 0))
# SAVE EDITOR: prata, com sombra escura e um leve brilho vermelho
x = xc + c.width + int(18 * escala); yt = yb + hb - int(31 * escala)
camada = Image.new('RGBA', img.size, (0, 0, 0, 0)); d = ImageDraw.Draw(camada)
sombra = Image.new('RGBA', img.size, (0, 0, 0, 0)); ds = ImageDraw.Draw(sombra)
xx = x
for ch in txt:
    ds.text((xx, yt), ch, font=f, fill=(150, 20, 10, 255))
    d.text((xx, yt), ch, font=f, fill=(226, 218, 204, 255))
    xx += f.getbbox(ch)[2] + esp
img.alpha_composite(sombra.filter(ImageFilter.GaussianBlur(3 * escala)))
img.alpha_composite(camada)
img.save(saida); print(saida, img.size)
