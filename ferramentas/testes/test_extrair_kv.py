#!/usr/bin/env python3
"""Testes do ferramentas/extrair-kv.py, só com NANDs sintéticas (nunca dados reais).

Uso: python3 ferramentas/testes/test_extrair_kv.py

Monta keyvaults e NANDs de teste de todos os formatos, com uma CPU key
inventada, e confere o programa de fora, como um usuário rodaria. O ECC das
cópias com spare é calculado pelo ecc_free60.c (a referência da Free60),
compilado na hora com o cc; sem compilador C, esses casos são pulados.
"""
import hashlib
import hmac
import importlib.util
import os
import random
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import zipfile

sys.dont_write_bytecode = True   # não deixa __pycache__ em ferramentas/

AQUI = os.path.dirname(os.path.abspath(__file__))
SCRIPT = os.path.join(AQUI, "..", "extrair-kv.py")
spec = importlib.util.spec_from_file_location("extrair_kv", SCRIPT)
KV = importlib.util.module_from_spec(spec)
spec.loader.exec_module(KV)

random.seed(360)
CPU = bytes(random.getrandbits(8) for _ in range(16))
OUTRA = bytes(random.getrandbits(8) for _ in range(16))
DVD = bytes(random.getrandbits(8) for _ in range(16))

falhas = 0
pulados = 0


def caso(nome, cond, saida=""):
    global falhas
    print(("  ok      " if cond else "  FALHOU  ") + nome)
    if not cond:
        falhas += 1
        print("          " + saida.replace("\n", "\n          "))


def xecrypt(numero, tamanho):
    be = numero.to_bytes(tamanho, "big")
    return b"".join(be[i:i + 8] for i in range(tamanho - 8, -1, -8))


def kv_de_teste():
    """Keyvault decifrado com série, certificado, chave privada (p × q = n) e o HMAC certo."""
    kv = bytearray(random.getrandbits(8) for _ in range(0x4000))
    kv[0xB0:0xBC] = b"123456789012"
    p = random.getrandbits(512) | 1 | (1 << 511)
    q = random.getrandbits(512) | 1 | (1 << 511)
    k = 0x298
    kv[k + 4:k + 8] = (65537).to_bytes(4, "big")
    kv[k + 0x10:k + 0x90] = xecrypt(p * q, 0x80)
    kv[k + 0x90:k + 0xD0] = xecrypt(p, 0x40)
    kv[k + 0xD0:k + 0x110] = xecrypt(q, 0x40)
    kv[0x9C8:0x9CA] = b"\x01\xa8"
    kv[0x9CF:0x9DA] = b"X999999-001"
    corpo = bytes(kv[0x10:])
    return hmac.new(CPU, corpo + b"\x07\x12", hashlib.sha1).digest()[:16] + corpo


KV_ORIGINAL = kv_de_teste()


def cifrar(kv):
    cabeca = kv[:16]
    return cabeca + KV.rc4(hmac.new(CPU, cabeca, hashlib.sha1).digest()[:16], kv[16:])


def imagem_logica(kv_pos=0x4000, cab_pos=0x4000, estragar=None):
    img = bytearray(kv_pos + 0x4000 + 0x8000)
    img[0:2] = b"\xff\x4f"
    img[0x60:0x64] = (0x4000).to_bytes(4, "big")
    img[0x6C:0x70] = cab_pos.to_bytes(4, "big")
    enc = bytearray(cifrar(KV_ORIGINAL))
    if estragar is not None:
        enc[estragar] ^= 0x55
    img[kv_pos:kv_pos + 0x4000] = enc
    return bytes(img)


def com_ecc(logica, ecc_ref):
    """Páginas com 16 bytes de spare e o ECC calculado pela referência em C."""
    paginas = []
    for n in range(0, len(logica), 0x200):
        spare = bytearray(16)
        spare[1] = (n // 0x4000) & 0xFF   # número do bloco
        spare[0xC] = 0x01                 # tipo do bloco, nos 6 bits de baixo
        paginas.append(logica[n:n + 0x200] + bytes(spare))
    saida = subprocess.run([ecc_ref], input=b"".join(paginas), capture_output=True, check=True)
    final = bytearray()
    for pagina, linha in zip(paginas, saida.stdout.decode().splitlines()):
        ecc = bytes.fromhex(linha.split()[1])
        pagina = bytearray(pagina)
        pagina[0x20C] = (pagina[0x20C] & 0x3F) | ecc[0]
        pagina[0x20D:0x210] = ecc[1:]
        final += pagina
    return bytes(final)


def arquivo(nome, conteudo, tamanho_total):
    with open(nome, "wb") as f:
        f.write(conteudo)
        f.truncate(tamanho_total)   # esparso: o resto não ocupa disco
    return nome


def rodar(*args, entrada=""):
    inicio = time.time()
    r = subprocess.run([sys.executable, SCRIPT, *args], capture_output=True, text=True, input=entrada)
    saida = r.stdout + r.stderr
    for chave in (CPU, OUTRA, DVD):
        assert chave.hex() not in saida.lower(), "uma chave apareceu na tela!"
    return r.returncode, saida, time.time() - inicio


def kv_igual(caminho="KV.bin"):
    return os.path.exists(caminho) and open(caminho, "rb").read() == KV_ORIGINAL


def apagar(*nomes):
    for nome in nomes:
        if os.path.isdir(nome):
            shutil.rmtree(nome)
        elif os.path.exists(nome):
            os.remove(nome)


def compilar_ecc(pasta):
    cc = shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")
    if not cc:
        return None
    destino = os.path.join(pasta, "ecc_free60")
    r = subprocess.run([cc, "-O2", "-o", destino, os.path.join(AQUI, "ecc_free60.c")], capture_output=True)
    return destino if r.returncode == 0 else None


def testar_ecc_contra_referencia(ecc_ref):
    """O ECC do programa tem que bater com a referência em C, e recusar 1 bit trocado."""
    paginas = [bytes(random.getrandbits(8) for _ in range(0x210)) for _ in range(200)]
    paginas += [bytes(0x210), b"\xff" * 0x210]
    saida = subprocess.run([ecc_ref], input=b"".join(paginas), capture_output=True, check=True).stdout.decode()
    bate = recusa = 0
    for pagina, linha in zip(paginas, saida.splitlines()):
        ecc = bytes.fromhex(linha.split()[1])
        p = bytearray(pagina)
        p[0x20C] = (p[0x20C] & 0x3F) | ecc[0]
        p[0x20D:0x210] = ecc[1:]
        bate += KV.ecc_confere(bytes(p))
        p[random.randrange(0x200)] ^= 1 << random.randrange(8)
        recusa += not KV.ecc_confere(bytes(p))
    caso("ECC igual ao da referência da Free60 em %d de %d páginas" % (bate, len(paginas)), bate == len(paginas))
    caso("recusa página com 1 bit trocado em %d de %d" % (recusa, len(paginas)), recusa == len(paginas))


def main():
    global pulados
    pasta = tempfile.mkdtemp(prefix="teste-kv-")
    os.chdir(pasta)
    try:
        ecc_ref = compilar_ecc(pasta)
        open("cpukey.txt", "w").write(CPU.hex().upper() + "\n")
        logica = imagem_logica()

        print("ECC:")
        if ecc_ref:
            testar_ecc_contra_referencia(ecc_ref)
        else:
            pulados += 1
            print("  pulado  sem compilador C para a referência do ECC")

        print("Formatos e tamanhos:")
        formatos = [
            ("eMMC de 48 MB (Corona 4 GB, Winchester)", logica, 48 << 20, "eMMC de 4 GB, cópia de 48 MB"),
            ("eMMC, imagem inteira de 4 GB", logica, 4 << 30, "eMMC de 4 GB, imagem inteira"),
            ("NAND de 16 MB sem ECC", logica, 16 << 20, "imagem de 16 MB, sem ECC"),
            ("cópia cortada, tamanho quebrado", logica, (3 << 20) + 123, "sem ECC"),
        ]
        if ecc_ref:
            raw = com_ecc(logica, ecc_ref)
            formatos += [
                ("NAND de 16 MB com ECC (Trinity, Corona 16 MB)", raw, 0x1080000, "NAND de 16 MB, com ECC"),
                ("NAND de 64 MB com ECC", raw, 0x4200000, "NAND de 64 MB, com ECC"),
                ("NAND de 256 MB com ECC (big block)", raw, 0x10800000, "NAND de 256 MB, com ECC"),
                ("NAND de 512 MB com ECC (big block)", raw, 0x21000000, "NAND de 512 MB, com ECC"),
            ]
        else:
            pulados += 4
            print("  pulado  NANDs com ECC (sem compilador C)")
        for nome, conteudo, tamanho, tipo in formatos:
            arquivo("nand.bin", conteudo, tamanho)
            cod, saida, dt = rodar("nand.bin", "cpukey.txt")
            ok = cod == 0 and tipo in saida and "confere (HMAC" in saida and kv_igual() and dt < 10
            if "com ECC" in tipo:
                ok = ok and "32 de 32 páginas do KV conferem" in saida
            caso("%s (%.2f s)" % (nome, dt), ok, saida)
            apagar("KV.bin", "nand.bin")

        print("Zip, pasta e CPU key:")
        arquivo("flashdmp.bin", logica, 48 << 20)
        with zipfile.ZipFile("backup.zip", "w", zipfile.ZIP_DEFLATED) as z:
            z.write("flashdmp.bin")
            z.write("cpukey.txt")
            z.writestr("foto.jpg", b"\xff\xd8\xff\xe0" + os.urandom(1 << 20))
        cod, saida, _ = rodar("backup.zip")
        caso("zip com flashdmp.bin, cpukey.txt e uma foto", cod == 0 and "flashdmp.bin" in saida and kv_igual(), saida)
        apagar("KV.bin")

        with zipfile.ZipFile("snf.zip", "w", zipfile.ZIP_DEFLATED) as z:
            z.write("flashdmp.bin", "recovery.bin")
            z.writestr("updflash.bin", logica[:0x200] + bytes(0x10000))
            z.writestr("Simple 360 NAND Flasher.log",
                       "Simple 360 NAND Flasher by Swizzy v1.4b (BETA)\n * Detected MMC NAND device!\n"
                       "DVD Key: %s\nYour CPUKey is: %s\n" % (DVD.hex().upper(), CPU.hex().upper()))
        cod, saida, _ = rodar("snf.zip")
        caso("zip do Simple 360 NAND Flasher: recovery.bin e a CPU key só no log",
             cod == 0 and "recovery.bin" in saida and kv_igual(), saida)
        apagar("KV.bin")

        with zipfile.ZipFile("espacos.zip", "w", zipfile.ZIP_DEFLATED) as z:
            z.write("flashdmp.bin", "nanddump1.bin")
            espacada = " ".join(CPU.hex().upper()[i:i + 8] for i in range(0, 32, 8))
            z.writestr("info.txt", "Hash: %s\nCPU Key: %s\n" % (OUTRA.hex() + "abcdef12", espacada))
        cod, saida, _ = rodar("espacos.zip")
        caso("CPU key com espaços, ao lado de um hash maior", cod == 0 and kv_igual(), saida)
        apagar("KV.bin")

        os.makedirs("pasta/sub")
        shutil.copy("flashdmp.bin", "pasta/sub/nanddump.bin")
        shutil.copy("cpukey.txt", "pasta/cpukey.txt")
        cod, saida, _ = rodar("pasta")
        caso("pasta com a NAND numa subpasta", cod == 0 and kv_igual(), saida)
        apagar("KV.bin")

        cod, saida, _ = rodar("flashdmp.bin")
        caso("só a NAND: acha o cpukey.txt na mesma pasta", cod == 0 and kv_igual(), saida)
        apagar("KV.bin")

        os.makedirs("sozinha")
        shutil.copy("flashdmp.bin", "sozinha/flashdmp.bin")
        cod, saida, _ = rodar("sozinha/flashdmp.bin")
        caso("sem CPU key e sem terminal: erro claro, nada gravado",
             cod == 1 and "não achei a CPU key" in saida and not os.path.exists("KV.bin"), saida)

        if os.name == "posix":
            testar_chave_digitada()

        print("Erros e proteções:")
        open("errada.txt", "w").write(OUTRA.hex())
        cod, saida, _ = rodar("flashdmp.bin", "errada.txt")
        caso("CPU key de outro console: não grava nada",
             cod == 1 and "não abriu" in saida and not os.path.exists("KV.bin"), saida)

        open("parecida.txt", "w").write(CPU.hex() + "abcdef12")
        cod, saida, _ = rodar("flashdmp.bin", "parecida.txt")
        caso("não pega 32 caracteres do meio de um hash maior", cod == 1 and not os.path.exists("KV.bin"), saida)

        arquivo("danificada.bin", imagem_logica(estragar=0x3000), 48 << 20)
        cod, saida, _ = rodar("danificada.bin", "cpukey.txt")
        caso("KV danificado: avisa e não grava", cod == 1 and "danificado" in saida and not os.path.exists("KV.bin"), saida)
        cod, saida, _ = rodar("danificada.bin", "cpukey.txt", "--gravar-danificado")
        caso("KV danificado com --gravar-danificado: grava e avisa",
             cod == 0 and "danificado" in saida and os.path.exists("KV.bin"), saida)
        apagar("KV.bin")

        if ecc_ref:
            ruim = bytearray(com_ecc(logica, ecc_ref))
            ruim[(0x4000 // 0x200 + 5) * 0x210 + 7] ^= 0x20
            arquivo("raw_ruim.bin", bytes(ruim), 0x1080000)
            cod, saida, _ = rodar("raw_ruim.bin", "cpukey.txt")
            caso("1 bit trocado numa página com ECC: diz quantas páginas estão erradas",
                 cod == 1 and "1 página(s) do KV estão com o ECC errado" in saida, saida)

        arquivo("cab_ruim.bin", imagem_logica(cab_pos=0xFFFFFFFF), 48 << 20)
        cod, saida, _ = rodar("cab_ruim.bin", "cpukey.txt")
        caso("cabeçalho com posição absurda: usa 0x4000", cod == 0 and "padrão" in saida and kv_igual(), saida)
        apagar("KV.bin")

        arquivo("kv8000.bin", imagem_logica(kv_pos=0x8000, cab_pos=0x8000), 48 << 20)
        cod, saida, _ = rodar("kv8000.bin", "cpukey.txt")
        caso("KV em outra posição: segue o cabeçalho",
             cod == 0 and "posição 0x8000, lida do cabeçalho" in saida and kv_igual(), saida)
        apagar("KV.bin")

        open("KV.bin", "wb").write(b"antigo")
        cod, saida, _ = rodar("flashdmp.bin", "cpukey.txt")
        caso("já existe KV.bin: não mexe nele",
             cod == 1 and "já existe" in saida and open("KV.bin", "rb").read() == b"antigo", saida)
        apagar("KV.bin")

        cod, saida, _ = rodar("flashdmp.bin", "cpukey.txt", "-o", "console/kv.bin")
        modo = stat.S_IMODE(os.stat("console/kv.bin").st_mode) if os.path.exists("console/kv.bin") else None
        ok = cod == 0 and kv_igual("console/kv.bin") and (os.name != "posix" or modo == 0o600)
        caso("-o console/kv.bin: cria a pasta, permissão 600", ok, "%s modo=%s" % (saida, modo))

        os.makedirs("saida")
        cod, saida, _ = rodar("flashdmp.bin", "cpukey.txt", "-o", "saida")
        caso("-o com uma pasta: grava KV.bin dentro", cod == 0 and kv_igual("saida/KV.bin"), saida)

        cod, saida, _ = rodar("nao-existe.bin", "cpukey.txt")
        caso("arquivo que não existe: erro sem traceback",
             cod == 1 and "Traceback" not in saida and "não achei" in saida, saida)
        arquivo("pequeno.bin", logica[:0x5000], 0x5000)
        cod, saida, _ = rodar("pequeno.bin", "cpukey.txt")
        caso("cópia pequena demais: erro claro", cod == 1 and "pequena demais" in saida, saida)
        open("falso.zip", "wb").write(b"PK\x03\x04" + os.urandom(100))
        cod, saida, _ = rodar("falso.zip", "cpukey.txt")
        caso("zip estragado: erro sem traceback", cod == 1 and "Traceback" not in saida, saida)
    finally:
        os.chdir(AQUI)
        shutil.rmtree(pasta, ignore_errors=True)

    print()
    print("Falhas: %d%s" % (falhas, ", pulados: %d" % pulados if pulados else ""))
    return 1 if falhas else 0


def testar_chave_digitada():
    """Num terminal de verdade, pede a CPU key sem mostrar o que foi digitado."""
    import pty
    import select
    pid, fd = pty.fork()
    if pid == 0:
        os.execv(sys.executable, [sys.executable, SCRIPT, "sozinha/flashdmp.bin"])
    tela = b""
    fim = time.time() + 20
    while time.time() < fim and b"CPU key (" not in tela:
        if select.select([fd], [], [], 0.2)[0]:
            tela += os.read(fd, 4096)
    os.write(fd, (CPU.hex().upper() + "\n").encode())
    while time.time() < fim:
        if select.select([fd], [], [], 0.2)[0]:
            try:
                pedaco = os.read(fd, 4096)
            except OSError:
                break
            if not pedaco:
                break
            tela += pedaco
    os.waitpid(pid, 0)
    texto = tela.decode("utf-8", "replace")
    caso("pede a CPU key num terminal, sem mostrar o que foi digitado",
         kv_igual() and CPU.hex().upper() not in texto and CPU.hex() not in texto, texto)
    apagar("KV.bin")


if __name__ == "__main__":
    sys.exit(main())
