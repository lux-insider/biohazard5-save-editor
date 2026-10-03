#!/usr/bin/env python3
"""Extrai o KV.bin (keyvault) de uma cópia da NAND do Xbox 360.

Funciona com a cópia de qualquer placa, de qualquer tamanho:
  - NAND de 16, 64, 256 ou 512 MB, com ou sem os 16 bytes de ECC de cada
    página (Xenon, Zephyr, Falcon, Jasper, Trinity, Corona 16 MB);
  - eMMC de 4 GB, na cópia de 48 MB ou na imagem inteira (Corona 4 GB,
    Winchester).

Lê as cópias do Simple 360 NAND Flasher (flashdmp.bin, recovery.bin,
cpukey.txt e o log) e do J-Runner (nanddump*.bin e cpukey.txt).

Uso:
  python3 extrair-kv.py                            abre um menu que acha as cópias sozinho
                                                   (no Windows: dois cliques no arquivo)
  python3 extrair-kv.py "Backup NAND.zip"          lê direto do zip
  python3 extrair-kv.py pasta-do-backup            procura na pasta
  python3 extrair-kv.py flashdmp.bin cpukey.txt    arquivos soltos
  python3 extrair-kv.py flashdmp.bin               usa o cpukey.txt da mesma pasta,
                                                   ou pede a CPU key, sem mostrar

Opções:
  -o ARQUIVO              onde gravar (padrão: KV.bin na pasta atual; para a
                          versão normal do editor, use -o console/kv.bin)
  --gravar-danificado     grava mesmo se o KV estiver danificado

O KV só é gravado depois de conferido do mesmo jeito que o console confere:
o HMAC-SHA1 da CPU key tem que bater com o do keyvault. Tudo acontece neste
computador, nada vai para a internet, e a CPU key nunca aparece na tela.
"""

import argparse
import contextlib
import getpass
import hashlib
import hmac
import os
import re
import sys
import zipfile

VERSAO = "2.1"

PAGINA = 0x200            # dados de uma página da NAND
PAGINA_ECC = 0x210        # a mesma página com os 16 bytes de spare (ECC)
KV_TAMANHO = 0x4000       # o keyvault tem sempre 16 KB
KV_PADRAO = 0x4000        # e fica nesta posição em todos os consoles conhecidos
KV_MAXIMO = 0x100000      # posição máxima aceita no cabeçalho
LEITURA = 0x120000        # basta ler o começo da cópia, nunca o arquivo todo
TEXTO_MAXIMO = 64 * 1024  # arquivos de texto maiores que isso não têm CPU key
NAND_MINIMA = 0x8000      # cabeçalho + keyvault
CHAVES_MAXIMO = 256

APENDICE_HMAC = b"\x07\x12"   # o console acrescenta estes 2 bytes no HMAC do KV

# Posições dentro do keyvault decifrado (libxenon, kvlookup)
SERIE = slice(0xB0, 0xBC)
CHAVE_PRIVADA = slice(0x298, 0x298 + 0x1D0)
CERTIFICADO = 0x9C8
PECA = slice(0x9CF, 0x9CF + 0xB)

MAGIC = b"\xff\x4f"           # começo de toda NAND de console comum

HEX32 = re.compile(rb"(?<![0-9A-Fa-f])[0-9A-Fa-f]{32}(?![0-9A-Fa-f])")
SEPARADORES = re.compile(rb"[ \t:\-]")


class Erro(Exception):
    """Erro com mensagem para o usuário, sem traceback."""


# --------------------------------------------------------------- criptografia

def rc4(chave, dados):
    s = list(range(256))
    j = 0
    for i in range(256):
        j = (j + s[i] + chave[i % len(chave)]) & 0xFF
        s[i], s[j] = s[j], s[i]
    saida = bytearray(len(dados))
    i = j = 0
    for n, b in enumerate(dados):
        i = (i + 1) & 0xFF
        j = (j + s[i]) & 0xFF
        s[i], s[j] = s[j], s[i]
        saida[n] = b ^ s[(s[i] + s[j]) & 0xFF]
    return bytes(saida)


def decifrar(cifrado, cpu_key):
    """Decifra o keyvault e confere o HMAC, como o console faz (libxenon, kv_read).

    Os 16 primeiros bytes são o HMAC-SHA1 do resto do KV decifrado, com a CPU
    key como chave. Deles sai também a chave do RC4 que cifra o resto.
    """
    cabeca = cifrado[:0x10]
    chave_rc4 = hmac.new(cpu_key, cabeca, hashlib.sha1).digest()[:0x10]
    kv = cabeca + rc4(chave_rc4, cifrado[0x10:])
    esperado = hmac.new(cpu_key, kv[0x10:] + APENDICE_HMAC, hashlib.sha1).digest()[:0x10]
    return kv, hmac.compare_digest(esperado, cabeca)


def numero_xecrypt(b):
    """Número grande do XeCrypt: palavras de 8 bytes, da menos para a mais significativa."""
    return int.from_bytes(b"".join(b[i:i + 8] for i in range(len(b) - 8, -1, -8)), "big")


def estrutura_confere(kv):
    """Confere a série, o certificado e a chave privada do console (p × q = n)."""
    k = kv[CHAVE_PRIVADA]
    n, p, q = numero_xecrypt(k[0x10:0x90]), numero_xecrypt(k[0x90:0xD0]), numero_xecrypt(k[0xD0:0x110])
    return (kv[SERIE].isdigit()
            and kv[CERTIFICADO:CERTIFICADO + 2] == b"\x01\xa8"
            and p > 1 and q > 1 and p * q == n)


def ecc_confere(pagina):
    """Confere os 26 bits de ECC de uma página (512 bytes de dados + 16 de spare).

    Algoritmo da Free60 (NAND File System): os 0x1066 primeiros bits da página,
    em palavras de 32 bits little-endian invertidas, passam por um LFSR.
    """
    val = 0
    v = 0
    for i in range(0x1066):
        if not i & 31:
            p = i >> 3
            v = ~int.from_bytes(pagina[p:p + 4], "little") & 0xFFFFFFFF
        val ^= v & 1
        v >>= 1
        if val & 1:
            val ^= 0x6954559
        val >>= 1
    val = ~val & 0xFFFFFFFF
    s = pagina[PAGINA:]
    return (s[0xC] & 0xC0 == (val << 6) & 0xC0 and s[0xD] == (val >> 2) & 0xFF
            and s[0xE] == (val >> 10) & 0xFF and s[0xF] == (val >> 18) & 0xFF)


# ------------------------------------------------------------ leitura da NAND

def ler_logico(prefixo, com_ecc, inicio, tamanho):
    """Lê da imagem lógica (sem ECC). Devolve (dados, páginas com ECC errado)."""
    if not com_ecc:
        return prefixo[inicio:inicio + tamanho], 0
    dados = bytearray()
    ruins = 0
    for pagina in range(inicio // PAGINA, (inicio + tamanho + PAGINA - 1) // PAGINA):
        bloco = prefixo[pagina * PAGINA_ECC:(pagina + 1) * PAGINA_ECC]
        if len(bloco) < PAGINA_ECC:
            break
        if not ecc_confere(bloco):
            ruins += 1
        dados += bloco[:PAGINA]
    resto = inicio % PAGINA
    return bytes(dados[resto:resto + tamanho]), ruins


def posicao_do_kv(cabecalho):
    """Posição do keyvault no cabeçalho da NAND (0x60: tamanho, 0x6C: posição)."""
    tamanho = int.from_bytes(cabecalho[0x60:0x64], "big")
    inicio = int.from_bytes(cabecalho[0x6C:0x70], "big")
    if tamanho == KV_TAMANHO and inicio % PAGINA == 0 and PAGINA <= inicio <= KV_MAXIMO:
        return inicio, True
    return KV_PADRAO, False


def descrever(tamanho, com_ecc):
    if com_ecc:
        mb = tamanho // PAGINA_ECC * PAGINA // (1 << 20)
        dicas = {16: " (Xenon a Jasper, Trinity, Corona 16 MB)",
                 64: " (NAND de 64 MB, ou o começo de uma big block)",
                 256: " (Jasper big block)", 512: " (Jasper big block)"}
        return "NAND de %d MB, com ECC%s" % (mb, dicas.get(mb, ""))
    mb = tamanho / (1 << 20)
    if mb == 48:
        return "eMMC de 4 GB, cópia de 48 MB (Corona 4 GB, Winchester)"
    if mb >= 4000:
        return "eMMC de 4 GB, imagem inteira (Corona 4 GB, Winchester)"
    return "imagem de %.0f MB, sem ECC" % mb


# ------------------------------------------------- de onde vêm os arquivos

class Arquivo:
    """Um arquivo solto, de uma pasta ou de dentro de um zip."""

    def __init__(self, nome, tamanho, abrir):
        self.nome = nome
        self.tamanho = tamanho
        self._abrir = abrir

    def ler(self, limite):
        try:
            with self._abrir() as f:
                return f.read(limite)
        except RuntimeError as e:  # zipfile: membro com senha
            raise Erro("o zip tem senha (%s). Extraia os arquivos e rode de novo com eles soltos." % e)


def limpo(nome):
    """Nome para mostrar na tela, sem caracteres de controle."""
    return "".join(c if c.isprintable() else "?" for c in nome)


def do_disco(caminho):
    return Arquivo(os.path.basename(caminho), os.path.getsize(caminho), lambda: open(caminho, "rb"))


def da_pasta(pasta):
    achados = []
    for raiz, subpastas, nomes in os.walk(pasta):
        if os.path.abspath(raiz) != os.path.abspath(pasta):
            subpastas[:] = []          # só a pasta e as subpastas dela, sem descer mais
        for nome in nomes:
            caminho = os.path.join(raiz, nome)
            if os.path.isfile(caminho) and not os.path.islink(caminho):
                achados.append(do_disco(caminho))
    return achados


def do_zip(caminho, pilha):
    try:
        z = pilha.enter_context(zipfile.ZipFile(caminho))
    except zipfile.BadZipFile:
        raise Erro("o arquivo não é um zip válido: %s" % caminho)
    return [Arquivo(os.path.basename(i.filename), i.file_size, lambda i=i: z.open(i))
            for i in z.infolist() if not i.is_dir()]


def escolher_nand(arquivos):
    """A cópia da NAND: começa com 0xFF e tem pelo menos o keyvault."""
    def nota(a):
        nome = a.nome.lower()
        # flashdmp.bin e recovery.bin: cópias do Simple 360 NAND Flasher; nanddump: J-Runner
        preferido = any(p in nome for p in ("flashdmp", "nanddump", "recovery", "nand", "orig"))
        return (preferido, "updflash" not in nome, a.tamanho)
    grandes = [a for a in arquivos if a.tamanho >= NAND_MINIMA]
    comecos = {id(a): a.ler(2) for a in grandes}
    candidatos = ([a for a in grandes if comecos[id(a)] == MAGIC]
                  or [a for a in grandes if comecos[id(a)][:1] == b"\xff"])
    if not candidatos:
        raise Erro("não achei uma cópia da NAND (um arquivo que comece com os bytes FF 4F).")
    return max(candidatos, key=nota)


def chaves_do_texto(texto):
    """Todas as sequências de 32 caracteres hexadecimais, também com espaços ou traços no meio."""
    achadas = [m.group() for m in HEX32.finditer(texto)]
    for linha in texto.splitlines():
        achadas += [m.group() for m in HEX32.finditer(SEPARADORES.sub(b"", linha))]
    return list(dict.fromkeys(bytes.fromhex(h.decode()) for h in achadas))


def chaves_dos_arquivos(arquivos, nand):
    nomeados = [a for a in arquivos if "cpu" in a.nome.lower() and "key" in a.nome.lower()]
    textos = [a for a in arquivos if a is not nand and a not in nomeados and a.tamanho <= TEXTO_MAXIMO
              and a.nome.lower().endswith((".txt", ".log", ".ini", ".cfg", ".json"))]
    chaves = []
    for a in nomeados + textos:
        chaves += chaves_do_texto(a.ler(TEXTO_MAXIMO))
    return chaves


def pedir_chave():
    if not sys.stdin.isatty():
        raise Erro("não achei a CPU key. Passe o arquivo dela depois da NAND: extrair-kv.py NAND cpukey.txt")
    texto = getpass.getpass("CPU key (32 caracteres, não aparece na tela): ").encode()
    chaves = chaves_do_texto(texto)
    if not chaves:
        raise Erro("isso não é uma CPU key: ela tem 32 caracteres de 0 a 9 e de A a F.")
    return chaves


# ------------------------------------------------------------------ gravação

def gravar(destino, kv):
    """Grava sem nunca sobrescrever, só para o dono, e confere o que foi gravado."""
    flags = (os.O_WRONLY | os.O_CREAT | os.O_EXCL
             | getattr(os, "O_BINARY", 0) | getattr(os, "O_NOINHERIT", 0))
    pasta = os.path.dirname(destino)
    try:
        if pasta and not os.path.isdir(pasta):
            os.makedirs(pasta, 0o700)        # por exemplo, -o console/kv.bin
        fd = os.open(destino, flags, 0o600)
    except FileExistsError:
        raise Erro("já existe %s. Mova ou renomeie esse arquivo antes, para não perder nenhum KV." % destino)
    except OSError as e:
        raise Erro("não consegui criar %s: %s" % (destino, e.strerror))
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(kv)
            f.flush()
            os.fsync(f.fileno())
        with open(destino, "rb") as f:
            if f.read() != kv:
                raise Erro("o arquivo gravado não ficou igual ao keyvault. Confira o disco.")
    except BaseException:
        try:
            os.unlink(destino)
        except OSError:
            pass
        raise


# ------------------------------------------------------- abrir uma cópia

def abrir_origem(origem, pilha):
    """A cópia da NAND e os arquivos ao lado dela, de um zip, de uma pasta ou de um arquivo."""
    if not os.path.exists(origem):
        raise Erro("não achei: %s" % origem)
    if os.path.isdir(origem):
        arquivos = da_pasta(origem)
        return escolher_nand(arquivos), arquivos
    if zipfile.is_zipfile(origem):
        arquivos = do_zip(origem, pilha)
        return escolher_nand(arquivos), arquivos
    nand = do_disco(origem)
    pasta = os.path.dirname(os.path.abspath(origem))
    arquivos = [do_disco(e.path) for e in os.scandir(pasta)   # a CPU key costuma estar ao lado
                if e.is_file(follow_symlinks=False) and e.path != os.path.abspath(origem)]
    return nand, arquivos


def abrir_kv(prefixo, tamanho, chaves):
    """Procura o keyvault na cópia e tenta cada CPU key.

    Devolve (achado, danificado): cada um é None ou (com_ecc, posição,
    posição veio do cabeçalho, páginas com ECC errado, kv decifrado).
    """
    com_ecc_primeiro = tamanho % PAGINA_ECC == 0
    achado = danificado = None
    for com_ecc in (com_ecc_primeiro, not com_ecc_primeiro):
        cabecalho, _ = ler_logico(prefixo, com_ecc, 0, PAGINA)
        inicio, do_cabecalho = posicao_do_kv(cabecalho)
        posicoes = [(inicio, do_cabecalho)] + ([(KV_PADRAO, False)] if inicio != KV_PADRAO else [])
        for inicio, do_cabecalho in posicoes:
            cifrado, ruins = ler_logico(prefixo, com_ecc, inicio, KV_TAMANHO)
            if len(cifrado) < KV_TAMANHO:
                continue
            for chave in chaves:
                kv, confere = decifrar(cifrado, chave)
                info = (com_ecc, inicio, do_cabecalho, ruins, kv)
                if confere:
                    return info, danificado
                if danificado is None and estrutura_confere(kv):
                    danificado = info
    return achado, danificado


def serie_e_peca(kv):
    serie = limpo(kv[SERIE].decode("ascii", "replace"))
    peca = kv[PECA]
    return serie, (peca.decode() if all(32 <= b < 127 for b in peca) else "")


# --------------------------------------------------------------------- main

def argumentos():
    ap = argparse.ArgumentParser(
        prog="extrair-kv.py", add_help=False,
        usage="%(prog)s [ORIGEM [CPUKEY]] [-o ARQUIVO] [--gravar-danificado]",
        description="Extrai o KV.bin (keyvault) da cópia da NAND do Xbox 360.")
    ap.add_argument("origem", nargs="?")
    ap.add_argument("cpukey", nargs="?")
    ap.add_argument("-o", "--saida", default="KV.bin")
    ap.add_argument("--gravar-danificado", action="store_true")
    ap.add_argument("-h", "--help", action="store_true")
    ap.add_argument("--version", action="version", version="extrair-kv.py " + VERSAO)
    a = ap.parse_args()
    a.menu = False
    if a.help:
        print(__doc__.strip())
        sys.exit(0)
    if not a.origem:
        if not sys.stdin.isatty():
            print(__doc__.strip())
            sys.exit(2)
        a.menu = True
    return a


def executar(a, pilha):
    nand, arquivos = abrir_origem(a.origem, pilha)

    if a.cpukey:
        if not os.path.isfile(a.cpukey):
            raise Erro("não achei o arquivo da CPU key: %s" % a.cpukey)
        chaves = chaves_do_texto(do_disco(a.cpukey).ler(TEXTO_MAXIMO))
        if not chaves:
            raise Erro("não achei uma CPU key (32 caracteres hexadecimais) em %s." % a.cpukey)
    else:
        chaves = chaves_dos_arquivos(arquivos, nand) or pedir_chave()
    chaves = list(dict.fromkeys(chaves))[:CHAVES_MAXIMO]
    if os.path.isdir(a.saida):
        a.saida = os.path.join(a.saida, "KV.bin")

    print("NAND:      %s (%.0f MB)" % (limpo(nand.nome), nand.tamanho / (1 << 20)))
    prefixo = nand.ler(LEITURA)
    if len(prefixo) < NAND_MINIMA:
        raise Erro("a cópia é pequena demais para ter o keyvault (%d bytes)." % len(prefixo))
    if prefixo[:2] != MAGIC:
        print("AVISO:     a cópia não começa com FF 4F, como a NAND de um console comum. Confira o arquivo.")

    achado, danificado = abrir_kv(prefixo, nand.tamanho, chaves)

    if not achado and not (danificado and a.gravar_danificado):
        if danificado:
            com_ecc, _, _, ruins, _ = danificado
            ecc = " %d página(s) do KV estão com o ECC errado." % ruins if com_ecc and ruins else ""
            raise Erro("a CPU key é deste console, mas o keyvault da cópia está danificado:"
                       " o HMAC não confere.%s Faça outra cópia da NAND. Nada foi gravado." % ecc)
        raise Erro("o keyvault não abriu com %s. Ou a CPU key não é deste console, ou a cópia não é"
                   " uma NAND do Xbox 360. Nada foi gravado."
                   % ("a CPU key" if len(chaves) == 1 else "nenhuma das %d CPU keys encontradas" % len(chaves)))

    com_ecc, inicio, do_cabecalho, ruins, kv = achado or danificado
    print("Tipo:      %s" % descrever(nand.tamanho, com_ecc))
    print("Keyvault:  posição 0x%X, %s" % (inicio, "lida do cabeçalho da NAND" if do_cabecalho
                                            else "a padrão (a do cabeçalho não servia)"))
    if com_ecc:
        paginas = KV_TAMANHO // PAGINA
        print("ECC:       %d de %d páginas do KV conferem" % (paginas - ruins, paginas))
    if achado:
        print("CPU key:   confere (HMAC-SHA1 igual ao que o console calcula)")
    else:
        print("AVISO:     o HMAC não confere: o KV está danificado. Gravado só porque você pediu.")
    serie, peca = serie_e_peca(kv)
    print("Série:     %s   <- tem que ser igual ao da etiqueta do console" % serie)
    if peca:
        print("Peça:      %s" % peca)

    gravar(a.saida, kv)
    dono = ", só você pode ler" if os.name == "posix" else ""
    print("Gravado:   %s (%d bytes)%s" % (os.path.abspath(a.saida), len(kv), dono))
    print("SHA-256:   %s" % hashlib.sha256(kv).hexdigest())


# --------------------------------------------------------------------- menu

LIMITE_BUSCA = 3000       # arquivos olhados no máximo em cada lugar
PROFUNDIDADE = 2          # a pasta, as subpastas e as subpastas delas
PULAR = {"node_modules", "$recycle.bin", "system volume information", "windows", "program files",
         "program files (x86)", "programdata", "appdata", "__pycache__"}
NOMES_EDITOR = ("biohazard5-save-editor", "biohazard 5 save editor")


class Cor:
    ligada = False

    @staticmethod
    def ligar():
        """Cores só num terminal de verdade; no console do Windows, liga o modo que entende as cores."""
        if not sys.stdout.isatty() or os.environ.get("NO_COLOR"):
            return
        if os.name == "nt":
            try:
                import ctypes
                k = ctypes.windll.kernel32
                saida = k.GetStdHandle(-11)
                modo = ctypes.c_uint32()
                if not k.GetConsoleMode(saida, ctypes.byref(modo)):
                    return
                if not k.SetConsoleMode(saida, modo.value | 0x0004):
                    return
            except Exception:
                return
        Cor.ligada = True


def cor(texto, codigo):
    return "\033[%sm%s\033[0m" % (codigo, texto) if Cor.ligada else texto


def titulo(texto):
    linha = "─" * 60
    print()
    print(cor(linha, "36"))
    print(cor("  " + texto, "1;36"))
    print(cor(linha, "36"))


def perguntar(texto):
    return input(cor(texto, "1")).strip()


def curto(caminho):
    """Caminho para mostrar: a pasta pessoal vira ~."""
    casa = os.path.expanduser("~")
    caminho = os.path.abspath(caminho)
    if casa and casa != os.sep and (caminho == casa or caminho.startswith(casa + os.sep)):
        caminho = "~" + caminho[len(casa):]
    return limpo(caminho)


def caminho_digitado(texto):
    """Caminho colado ou arrastado para o terminal: tira aspas e entende o ~."""
    texto = texto.strip()
    if len(texto) >= 2 and texto[0] == texto[-1] and texto[0] in "'\"":
        texto = texto[1:-1]
    return os.path.expanduser(texto.replace("\\ ", " ") if os.name != "nt" else texto)


def lugares_de_busca():
    """Onde costumam estar as cópias: Downloads, Área de Trabalho, Documentos e pendrives."""
    casa = os.path.expanduser("~")
    nomes = ["Downloads", "Transferências", "Desktop", "Área de Trabalho", "Documents", "Documentos",
             os.path.join("OneDrive", "Desktop"), os.path.join("OneDrive", "Área de Trabalho"),
             os.path.join("OneDrive", "Documentos"), os.path.join("OneDrive", "Documents")]
    lugares = [(os.path.join(casa, n), n) for n in nomes]
    lugares += pastas_xdg(casa)
    lugares.append((os.getcwd(), "pasta atual"))
    lugares.append((os.path.dirname(os.path.abspath(sys.argv[0])), "pasta do programa"))
    if os.name == "nt":
        for letra in "DEFGHIJKLMNOPQRSTUVWXYZ":     # pendrives e HDs externos
            raiz = letra + ":\\"
            if os.path.isdir(raiz):
                lugares.append((raiz, raiz))
    else:
        usuario = os.environ.get("USER") or os.path.basename(casa)
        for base in ("/media/" + usuario, "/run/media/" + usuario, "/media", "/mnt"):
            try:
                for e in sorted(os.scandir(base), key=lambda e: e.name):
                    if e.is_dir(follow_symlinks=False):
                        lugares.append((e.path, e.path))
            except OSError:
                pass
    vistos, saida = set(), []
    for caminho, nome in lugares:
        real = os.path.realpath(caminho)
        if os.path.isdir(real) and real not in vistos:
            vistos.add(real)
            saida.append((real, nome))
    return saida


def pastas_xdg(casa):
    """Downloads, Área de Trabalho e Documentos como o Linux configurou (user-dirs.dirs)."""
    pastas = []
    try:
        with open(os.path.join(casa, ".config", "user-dirs.dirs"), encoding="utf-8") as f:
            for linha in f:
                chave, _, valor = linha.strip().partition("=")
                if chave in ("XDG_DOWNLOAD_DIR", "XDG_DESKTOP_DIR", "XDG_DOCUMENTS_DIR") and valor:
                    valor = valor.strip('"').replace("$HOME", casa)
                    pastas.append((valor, os.path.basename(valor)))
    except OSError:
        pass
    return pastas


def eh_editor(nome):
    """O executável do editor: biohazard5-save-editor no Linux, BIOHAZARD 5 SAVE EDITOR*.exe no Windows."""
    base, ext = os.path.splitext(nome.lower())
    return ext in ("", ".exe") and any(base.startswith(n) for n in NOMES_EDITOR)


def andar(raiz):
    """Os arquivos da pasta, até PROFUNDIDADE níveis abaixo, sem seguir atalhos."""
    pilha = [(raiz, 0)]
    while pilha:
        pasta, nivel = pilha.pop()
        try:
            entradas = sorted(os.scandir(pasta), key=lambda e: e.name.lower())
        except OSError:
            continue
        for e in entradas:
            try:
                if e.is_symlink():
                    continue
                if e.is_dir():
                    if nivel < PROFUNDIDADE and not e.name.startswith(".") and e.name.lower() not in PULAR:
                        pilha.append((e.path, nivel + 1))
                elif e.is_file():
                    yield e
            except OSError:
                continue


def parece_nand(cabeca):
    """Começa com FF 4F e o cabeçalho diz que o keyvault tem 16 KB: é uma NAND do Xbox 360."""
    return (len(cabeca) >= 0x70 and cabeca[:2] == MAGIC
            and (int.from_bytes(cabeca[0x60:0x64], "big") == KV_TAMANHO
                 or int.from_bytes(cabeca[0x6C:0x70], "big") == KV_PADRAO))


class Copia:
    """Uma cópia da NAND achada no computador, com o que dá para saber dela."""

    def __init__(self, origem, lugar, nand, arquivos):
        self.origem = origem
        self.lugar = lugar
        self.nand = nand
        self.arquivos = arquivos
        self.com_ecc = None
        self.achado = self.danificado = None
        self.tem_chave = False

    def identificar(self, chaves=None):
        """Lê o começo da cópia e tenta abrir o KV com as CPU keys que estão junto."""
        if chaves is None:
            chaves = chaves_dos_arquivos(self.arquivos, self.nand)[:CHAVES_MAXIMO]
            self.tem_chave = bool(chaves)
        prefixo = self.nand.ler(LEITURA)
        self.com_ecc = self.nand.tamanho % PAGINA_ECC == 0
        if len(prefixo) >= NAND_MINIMA and chaves:
            self.achado, self.danificado = abrir_kv(prefixo, self.nand.tamanho, chaves)
            info = self.achado or self.danificado
            if info:
                self.com_ecc = info[0]

    def linhas(self):
        tipo = descrever(self.nand.tamanho, self.com_ecc)
        if self.achado:
            serie, peca = serie_e_peca(self.achado[4])
            situacao = cor("série %s%s · CPU key confere" % (serie, " · peça " + peca if peca else ""), "32")
        elif self.danificado:
            situacao = cor("a CPU key é deste console, mas o KV da cópia está danificado", "31")
        elif self.tem_chave:
            situacao = cor("a CPU key que está junto não abre esta cópia", "33")
        else:
            situacao = cor("sem a CPU key junto: ela vai ser pedida", "2")
        return tipo, situacao


def procurar(pilha):
    """Procura cópias da NAND e o editor nos lugares de costume. Reconhece pelo conteúdo, não pelo nome."""
    copias, editores, vistos = [], [], set()
    for raiz, lugar in lugares_de_busca():
        olhados = 0
        for e in andar(raiz):
            olhados += 1
            if olhados > LIMITE_BUSCA:
                break
            nome = e.name.lower()
            if eh_editor(e.name):
                pasta = os.path.dirname(e.path)
                if pasta not in editores:
                    editores.append(pasta)
                continue
            real = os.path.realpath(e.path)
            if real in vistos:
                continue
            try:
                tamanho = e.stat().st_size
            except OSError:
                continue
            if tamanho < NAND_MINIMA:
                continue
            onde = os.path.dirname(e.path)
            if nome.endswith(".zip"):
                try:
                    arquivos = do_zip(e.path, pilha)
                    # a NAND é um dos arquivos grandes: olha só os 10 maiores do zip
                    grandes = sorted((a for a in arquivos if a.tamanho >= NAND_MINIMA),
                                     key=lambda a: a.tamanho, reverse=True)[:10]
                    if not any(parece_nand(a.ler(0x70)) for a in grandes):
                        continue
                    nand = escolher_nand(arquivos)
                except (Erro, OSError, zipfile.BadZipFile, RuntimeError, ValueError):
                    continue
            else:
                try:
                    with open(e.path, "rb") as f:
                        if not parece_nand(f.read(0x70)):
                            continue
                except OSError:
                    continue
                nand = do_disco(e.path)
                try:
                    arquivos = [do_disco(v.path) for v in os.scandir(onde)
                                if v.is_file(follow_symlinks=False) and v.path != e.path]
                except OSError:
                    arquivos = []
            vistos.add(real)
            copias.append(Copia(e.path, onde, nand, arquivos))
    for c in copias:
        try:
            c.identificar()
        except (Erro, OSError, RuntimeError, ValueError):
            pass
    return copias, editores


def mostrar_copias(copias):
    if not copias:
        print("  Não achei nenhuma cópia da NAND em Downloads, na Área de Trabalho, em Documentos")
        print("  nem nos pendrives. Use a opção [c] para dizer onde ela está.")
    for n, c in enumerate(copias, 1):
        tipo, situacao = c.linhas()
        print()
        print("  %s %s" % (cor("[%d]" % n, "1;36"), cor(limpo(os.path.basename(c.origem)), "1")))
        print("      em %s" % curto(c.lugar))
        print("      %s" % tipo)
        print("      %s" % situacao)
    print()
    print("  %s digitar ou arrastar para cá o caminho de outra cópia (zip, pasta ou arquivo)" % cor("[c]", "1;36"))
    print("  %s sair" % cor("[s]", "1;36"))
    print()


def escolher_copia(copias, pilha):
    while True:
        resposta = perguntar("Escolha a cópia: ").lower()
        if resposta in ("s", "sair", "0"):
            return None
        if resposta == "c":
            caminho = caminho_digitado(perguntar("Caminho da cópia: "))
            if not caminho:
                continue
            try:
                nand, arquivos = abrir_origem(caminho, pilha)
            except Erro as e:
                print(cor("  %s" % e, "31"))
                continue
            c = Copia(caminho, os.path.dirname(os.path.abspath(caminho)), nand, arquivos)
            try:
                c.identificar()
            except (Erro, OSError, RuntimeError, ValueError) as e:
                print(cor("  não consegui ler a cópia: %s" % e, "31"))
                continue
            return c
        if resposta.isdigit() and 1 <= int(resposta) <= len(copias):
            return copias[int(resposta) - 1]
        print(cor("  Digite o número de uma cópia, c ou s.", "33"))


def garantir_kv(c):
    """O KV conferido da cópia escolhida; pede a CPU key se ela não estava junto."""
    if c.achado:
        return c.achado
    if c.danificado:
        com_ecc, _, _, ruins, _ = c.danificado
        ecc = " %d página(s) do KV estão com o ECC errado." % ruins if com_ecc and ruins else ""
        print(cor("  A CPU key é deste console, mas o KV desta cópia está danificado: o HMAC não confere.%s"
                  % ecc, "31"))
        print("  Faça outra cópia da NAND no console. Nada foi gravado.")
        return None
    if c.tem_chave:
        print(cor("  A CPU key que está junto com esta cópia não abre o KV.", "33"))
    while True:
        texto = getpass.getpass("  Digite a CPU key (32 caracteres, não aparece na tela; ENTER volta): ")
        if not texto.strip():
            return None
        chaves = chaves_do_texto(texto.encode())
        if not chaves:
            print(cor("  Isso não é uma CPU key: ela tem 32 caracteres de 0 a 9 e de A a F.", "33"))
            continue
        c.identificar(chaves)
        if c.achado:
            return c.achado
        if c.danificado:
            return garantir_kv(c)
        print(cor("  Essa CPU key não abre o KV desta cópia. Confira se ela é deste console.", "33"))


def escolher_destino(c, kv, editores):
    serie, _ = serie_e_peca(kv)
    pasta_da_copia = os.path.dirname(os.path.abspath(c.origem))
    opcoes = [(os.path.join(p, "console", "kv.bin"), "na pasta do editor") for p in editores]
    opcoes.append((os.path.join(pasta_da_copia, "KV-%s.bin" % serie), "ao lado da cópia"))
    while True:
        print()
        print(cor("  Onde gravar o kv.bin?", "1"))
        for n, (caminho, rotulo) in enumerate(opcoes, 1):
            print("  %s %s: %s" % (cor("[%d]" % n, "1;36"), rotulo, curto(caminho)))
        print("  %s na pasta do editor, digitando ou arrastando a pasta dele" % cor("[e]", "1;36"))
        print("  %s em outra pasta" % cor("[o]", "1;36"))
        print("  %s voltar" % cor("[v]", "1;36"))
        resposta = perguntar("Escolha: ").lower()
        if resposta in ("v", "voltar"):
            return None
        if resposta.isdigit() and 1 <= int(resposta) <= len(opcoes):
            return opcoes[int(resposta) - 1][0]
        if resposta in ("e", "o"):
            pasta = caminho_digitado(perguntar("Pasta do editor: " if resposta == "e" else "Pasta: "))
            if not pasta:
                continue
            if not os.path.isdir(pasta):
                print(cor("  Essa pasta não existe: %s" % pasta, "33"))
                continue
            if resposta == "e":
                return os.path.join(pasta, "console", "kv.bin")
            return os.path.join(pasta, "KV-%s.bin" % serie)
        print(cor("  Digite o número de uma opção, e, o ou v.", "33"))


def menu():
    Cor.ligar()
    titulo("Extrair o kv.bin do Xbox 360  ·  versão %s" % VERSAO)
    print("  A CPU key nunca aparece na tela, e nada vai para a internet.")
    print("  Procurando cópias da NAND em Downloads, Área de Trabalho, Documentos e pendrives...")
    with contextlib.ExitStack() as pilha:
        copias, editores = procurar(pilha)
        while True:
            titulo("Cópias da NAND encontradas")
            mostrar_copias(copias)
            c = escolher_copia(copias, pilha)
            if c is None:
                return
            info = garantir_kv(c)
            if not info:
                continue
            com_ecc, _, _, ruins, kv = info
            serie, peca = serie_e_peca(kv)
            print()
            print(cor("  CPU key confere. Série %s%s." % (serie, ", peça " + peca if peca else ""), "32"))
            print("  Confira se a série é a mesma da etiqueta do console.")
            while True:
                destino = escolher_destino(c, kv, editores)
                if destino is None:
                    break
                try:
                    gravar(destino, kv)
                except Erro as e:
                    print(cor("  %s" % e, "31"))
                    continue
                titulo("Pronto!")
                print("  kv.bin gravado em: %s" % curto(destino))
                print("  SHA-256: %s" % hashlib.sha256(kv).hexdigest())
                print("  Guarde o kv.bin, a cópia da NAND e a CPU key num lugar seguro, com senha.")
                return


def main():
    a = argumentos()
    if a.menu:
        try:
            menu()
        except (KeyboardInterrupt, EOFError):
            print("\nCancelado. Nada foi gravado.")
        except (Erro, OSError) as e:
            print(cor("\nERRO: %s" % e, "31"))
        try:
            input("\nAperte ENTER para sair.")
        except (KeyboardInterrupt, EOFError):
            pass
        return
    try:
        with contextlib.ExitStack() as pilha:
            executar(a, pilha)
    except Erro as e:
        print("ERRO: %s" % e, file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print("\nCancelado. Nada foi gravado.", file=sys.stderr)
        sys.exit(130)
    except OSError as e:
        print("ERRO: %s: %s" % (e.filename or "arquivo", e.strerror or e), file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
