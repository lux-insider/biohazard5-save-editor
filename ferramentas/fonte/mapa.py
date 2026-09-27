"""Célula da imagem font00 (36x36, 64 por linha) -> caractere. Código MSG2 = célula * 2."""
MAQUINA = {}   # estilo máquina de escrever
for i, c in enumerate('0123456789!?() &:;,."\'~-+/@$'):
    MAQUINA[i] = c
for i in range(26):
    MAQUINA[28 + i] = chr(65 + i)
    MAQUINA[54 + i] = chr(97 + i)
for i, c in enumerate('[]¡¿®°ÀÁÂÄÇÈÉÊËÌÍÎÏÑÒÓÔÖÚÛÜßàáâäçèéêëìíîïñòóôöùúûüŒœ'):
    MAQUINA[80 + i] = c
for cel, c in {133: '×', 134: '=', 135: '„', 136: 'Ù', 137: 'º', 138: 'ª', 139: '%', 140: '|', 141: '_', 142: '>'}.items():
    MAQUINA[cel] = c

SERIFA = {}
for i, c in enumerate('0123456789!?()'):
    SERIFA[144 + i] = c
for i, c in enumerate('&:;,.“”‘’'):
    SERIFA[159 + i] = c
for cel, c in {169: '-', 170: '+', 171: '×', 172: '÷', 174: '%'}.items():
    SERIFA[cel] = c
for i in range(26):
    SERIFA[175 + i] = chr(65 + i)
    SERIFA[201 + i] = chr(97 + i)

# Símbolos da linha dos kana (desenho único, entram nos dois estilos).
SIMBOLOS = {398: '▼', 399: '▲', 403: '◆', 404: '○', 405: '♪', 406: '↑', 407: '↓', 408: '→', 409: '←',
            410: '★', 411: '☆', 412: '<', 414: '=', 415: '—', 417: '∞', 419: '*', 420: '#', 422: '※',
            423: '©', 424: '·', 425: '…', 427: '™', 421: 'β'}

# Caracteres usados pelo editor que o jogo não tem: apontam para o desenho mais próximo.
APELIDOS = {'•': '·', '–': '-', '✕': '×', '✔': '◆', '⚠': '!'}
