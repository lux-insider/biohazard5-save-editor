//! Testes do extrair-kv, só com NANDs sintéticas (nunca dados reais).
//!
//! Monta keyvaults e NANDs de teste de todos os formatos, com uma CPU key
//! inventada, e confere o programa de fora, como um usuário rodaria. O ECC das
//! cópias com spare é calculado pelo ferramentas/testes/ecc_free60.c (a
//! referência da Free60), compilado na hora com o cc; sem compilador C, esses
//! casos são pulados. O menu e a CPU key digitada são testados num terminal de
//! verdade (pty), no Linux e no macOS.
//!
//!     cargo test --release -p extrair-kv
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use num_bigint::BigUint;
use sha1::{Digest, Sha1};

// ------------------------------------------------------------ dados de teste

/// Gerador de números (splitmix64): os mesmos dados em toda execução.
struct Aleatorio(u64);

impl Aleatorio {
    fn u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.u64() as u8).collect()
    }

    fn chave(&mut self) -> [u8; 16] {
        self.bytes(16).try_into().unwrap()
    }

    /// Número ímpar de 512 bits com o bit de cima ligado (como um primo da chave RSA).
    fn numero_512(&mut self) -> BigUint {
        let mut b = self.bytes(64);
        b[0] |= 0x80;
        b[63] |= 1;
        BigUint::from_bytes_be(&b)
    }
}

/// HMAC-SHA1 feito à parte do programa, para conferir o dele.
fn hmac_sha1(chave: &[u8], dados: &[u8]) -> [u8; 20] {
    let mut bloco = [0u8; 64];
    bloco[..chave.len()].copy_from_slice(chave);
    let interno = Sha1::new().chain_update(bloco.map(|b| b ^ 0x36)).chain_update(dados).finalize();
    Sha1::new().chain_update(bloco.map(|b| b ^ 0x5C)).chain_update(interno).finalize().into()
}

/// RC4 feito à parte do programa.
fn rc4(chave: &[u8], dados: &[u8]) -> Vec<u8> {
    let mut s: Vec<u8> = (0..=255).collect();
    let mut j = 0u8;
    for i in 0..256 {
        j = j.wrapping_add(s[i]).wrapping_add(chave[i % chave.len()]);
        s.swap(i, j as usize);
    }
    let (mut i, mut j) = (0u8, 0u8);
    dados
        .iter()
        .map(|b| {
            i = i.wrapping_add(1);
            j = j.wrapping_add(s[i as usize]);
            s.swap(i as usize, j as usize);
            b ^ s[s[i as usize].wrapping_add(s[j as usize]) as usize]
        })
        .collect()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Número grande no formato do XeCrypt: palavras de 8 bytes em ordem inversa.
fn xecrypt(n: &BigUint, tamanho: usize) -> Vec<u8> {
    let b = n.to_bytes_be();
    let mut be = vec![0u8; tamanho - b.len()];
    be.extend_from_slice(&b);
    be.chunks(8).rev().flatten().copied().collect()
}

/// Um console inventado: CPU key e keyvault decifrado com série, certificado,
/// chave privada (p × q = n) e o HMAC certo.
struct Console {
    cpu: [u8; 16],
    kv: Vec<u8>,
}

fn console(serie: &str, peca: &str, semente: u64) -> Console {
    let mut r = Aleatorio(semente);
    let cpu = r.chave();
    let mut kv = r.bytes(0x4000);
    kv[0xB0..0xBC].copy_from_slice(serie.as_bytes());
    let (p, q) = (r.numero_512(), r.numero_512());
    let k = 0x298;
    kv[k + 4..k + 8].copy_from_slice(&65537u32.to_be_bytes());
    kv[k + 0x10..k + 0x90].copy_from_slice(&xecrypt(&(&p * &q), 0x80));
    kv[k + 0x90..k + 0xD0].copy_from_slice(&xecrypt(&p, 0x40));
    kv[k + 0xD0..k + 0x110].copy_from_slice(&xecrypt(&q, 0x40));
    kv[0x9C8..0x9CA].copy_from_slice(&[0x01, 0xA8]);
    kv[0x9CF..0x9DA].copy_from_slice(peca.as_bytes());
    let mut corpo = kv[0x10..].to_vec();
    corpo.extend_from_slice(&[0x07, 0x12]);
    kv[..0x10].copy_from_slice(&hmac_sha1(&cpu, &corpo)[..0x10]);
    Console { cpu, kv }
}

impl Console {
    fn cifrado(&self) -> Vec<u8> {
        let cabeca = &self.kv[..0x10];
        let mut c = cabeca.to_vec();
        c.extend(rc4(&hmac_sha1(&self.cpu, cabeca)[..0x10], &self.kv[0x10..]));
        c
    }

    /// Cópia lógica (sem ECC) com o cabeçalho e o KV cifrado.
    fn imagem(&self, kv_pos: usize, cab_pos: u32, estragar: Option<usize>) -> Vec<u8> {
        let mut img = vec![0u8; kv_pos + 0x4000 + 0x8000];
        img[..2].copy_from_slice(&[0xFF, 0x4F]);
        img[0x60..0x64].copy_from_slice(&0x4000u32.to_be_bytes());
        img[0x6C..0x70].copy_from_slice(&cab_pos.to_be_bytes());
        let mut enc = self.cifrado();
        if let Some(i) = estragar {
            enc[i] ^= 0x55;
        }
        img[kv_pos..kv_pos + 0x4000].copy_from_slice(&enc);
        img
    }
}

/// O console dos testes de linha de comando.
fn principal() -> &'static Console {
    static C: OnceLock<Console> = OnceLock::new();
    C.get_or_init(|| console("123456789012", "X999999-001", 360))
}

fn outra_cpu() -> [u8; 16] {
    Aleatorio(1360).chave()
}

fn dvd_key() -> [u8; 16] {
    Aleatorio(2360).chave()
}

fn logica() -> &'static [u8] {
    static L: OnceLock<Vec<u8>> = OnceLock::new();
    L.get_or_init(|| principal().imagem(0x4000, 0x4000, None))
}

/// A referência do ECC da Free60 em C, compilada uma vez.
fn ecc_ref() -> Option<&'static Path> {
    static REF: OnceLock<Option<PathBuf>> = OnceLock::new();
    REF.get_or_init(|| {
        let fonte = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testes/ecc_free60.c");
        let destino = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ecc_free60-{}", std::process::id()));
        ["cc", "gcc", "clang"].iter().find_map(|cc| {
            let ok = Command::new(cc).args(["-O2", "-o"]).arg(&destino).arg(&fonte).status().is_ok_and(|s| s.success());
            ok.then(|| destino.clone())
        })
    })
    .as_deref()
}

fn pular(motivo: &str) {
    eprintln!("  pulado: {motivo}");
}

/// O ECC de cada página, calculado pela referência em C.
fn ecc_das_paginas(paginas: &[u8]) -> Vec<[u8; 4]> {
    let mut filho = Command::new(ecc_ref().unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut entrada = filho.stdin.take().unwrap();
    let dados = paginas.to_vec();
    let escritor = std::thread::spawn(move || std::io::Write::write_all(&mut entrada, &dados).unwrap());
    let saida = filho.wait_with_output().unwrap();
    escritor.join().unwrap();
    String::from_utf8(saida.stdout)
        .unwrap()
        .lines()
        .map(|l| {
            let h = l.split_whitespace().nth(1).unwrap();
            std::array::from_fn(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap())
        })
        .collect()
}

fn por_ecc(pagina: &mut [u8], ecc: [u8; 4]) {
    pagina[0x20C] = (pagina[0x20C] & 0x3F) | ecc[0];
    pagina[0x20D..0x210].copy_from_slice(&ecc[1..]);
}

/// Páginas com 16 bytes de spare e o ECC calculado pela referência em C.
fn com_ecc(logica: &[u8]) -> Vec<u8> {
    let mut paginas = Vec::new();
    for (n, dados) in logica.chunks(0x200).enumerate() {
        paginas.extend_from_slice(dados);
        let mut spare = [0u8; 16];
        spare[1] = (n * 0x200 / 0x4000) as u8; // número do bloco
        spare[0xC] = 0x01; // tipo do bloco, nos 6 bits de baixo
        paginas.extend_from_slice(&spare);
    }
    let eccs = ecc_das_paginas(&paginas);
    for (pagina, ecc) in paginas.chunks_mut(0x210).zip(eccs) {
        por_ecc(pagina, ecc);
    }
    paginas
}

// --------------------------------------------------------- pastas e arquivos

/// Pasta temporária, apagada no fim do teste.
struct Pasta(PathBuf);

impl Pasta {
    fn nova(nome: &str) -> Pasta {
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("teste-kv-{nome}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        Pasta(p)
    }

    fn join(&self, caminho: &str) -> PathBuf {
        self.0.join(caminho)
    }
}

impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Grava o conteúdo e estica o arquivo até o tamanho total (esparso: o resto não ocupa disco).
fn arquivo(caminho: &Path, conteudo: &[u8], tamanho_total: u64) {
    if let Some(pasta) = caminho.parent() {
        fs::create_dir_all(pasta).unwrap();
    }
    fs::write(caminho, conteudo).unwrap();
    fs::OpenOptions::new().write(true).open(caminho).unwrap().set_len(tamanho_total).unwrap();
}

fn texto(caminho: &Path, conteudo: &str) {
    fs::write(caminho, conteudo).unwrap();
}

fn cpukey_txt(pasta: &Pasta) {
    texto(&pasta.join("cpukey.txt"), &format!("{}\n", hex(&principal().cpu).to_uppercase()));
}

fn kv_igual(caminho: &Path) -> bool {
    fs::read(caminho).is_ok_and(|kv| kv == principal().kv)
}

/// CRC-32 feito à parte do programa.
fn crc32(dados: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in dados {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

/// Um zip simples (deflate ou sem compressão), feito à parte do programa.
#[derive(Default)]
struct ZipNovo {
    dados: Vec<u8>,
    central: Vec<u8>,
    n: u16,
}

impl ZipNovo {
    fn juntar(mut self, nome: &str, conteudo: &[u8]) -> Self {
        let comprimido = miniz_oxide::deflate::compress_to_vec(conteudo, 1);
        let (metodo, guardado) = if comprimido.len() < conteudo.len() { (8u16, comprimido) } else { (0, conteudo.to_vec()) };
        let flags: u16 = if nome.is_ascii() { 0 } else { 0x800 };
        let posicao = self.dados.len() as u32;
        let comum = |v: &mut Vec<u8>| {
            v.extend_from_slice(&20u16.to_le_bytes());
            v.extend_from_slice(&flags.to_le_bytes());
            v.extend_from_slice(&metodo.to_le_bytes());
            v.extend_from_slice(&[0, 0, 0x21, 0]); // hora e data: 1980-01-01
            v.extend_from_slice(&crc32(conteudo).to_le_bytes());
            v.extend_from_slice(&(guardado.len() as u32).to_le_bytes());
            v.extend_from_slice(&(conteudo.len() as u32).to_le_bytes());
            v.extend_from_slice(&(nome.len() as u16).to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
        };
        self.dados.extend_from_slice(b"PK\x03\x04");
        comum(&mut self.dados);
        self.dados.extend_from_slice(nome.as_bytes());
        self.dados.extend_from_slice(&guardado);
        self.central.extend_from_slice(b"PK\x01\x02");
        self.central.extend_from_slice(&20u16.to_le_bytes());
        comum(&mut self.central);
        self.central.extend_from_slice(&[0; 10]); // comentário, disco e atributos
        self.central.extend_from_slice(&posicao.to_le_bytes());
        self.central.extend_from_slice(nome.as_bytes());
        self.n += 1;
        self
    }

    fn gravar(mut self, caminho: &Path) {
        let inicio = self.dados.len() as u32;
        self.dados.extend_from_slice(&self.central);
        self.dados.extend_from_slice(b"PK\x05\x06\0\0\0\0");
        self.dados.extend_from_slice(&self.n.to_le_bytes());
        self.dados.extend_from_slice(&self.n.to_le_bytes());
        self.dados.extend_from_slice(&(self.central.len() as u32).to_le_bytes());
        self.dados.extend_from_slice(&inicio.to_le_bytes());
        self.dados.extend_from_slice(&0u16.to_le_bytes());
        fs::write(caminho, self.dados).unwrap();
    }
}

/// A cópia de 48 MB da eMMC (Corona 4 GB, Winchester), inteira na memória.
fn emmc_48(logica: &[u8]) -> Vec<u8> {
    let mut v = logica.to_vec();
    v.resize(48 << 20, 0);
    v
}

// ------------------------------------------------------------- o programa

struct Saida {
    codigo: i32,
    texto: String,
    segundos: f64,
}

fn rodar(pasta: &Pasta, args: &[&str]) -> Saida {
    let inicio = Instant::now();
    let r = Command::new(env!("CARGO_BIN_EXE_extrair-kv"))
        .args(args)
        .current_dir(&pasta.0)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let texto = format!("{}{}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr));
    for chave in [principal().cpu, outra_cpu(), dvd_key()] {
        assert!(!texto.to_lowercase().contains(&hex(&chave)), "uma chave apareceu na tela!");
    }
    Saida { codigo: r.status.code().unwrap_or(-1), texto, segundos: inicio.elapsed().as_secs_f64() }
}

// --------------------------------------------------------------------- ECC

#[test]
fn ecc_igual_ao_da_referencia_da_free60() {
    if ecc_ref().is_none() {
        return pular("sem compilador C para a referência do ECC");
    }
    let mut r = Aleatorio(0xECC);
    let mut paginas: Vec<Vec<u8>> = (0..200).map(|_| r.bytes(0x210)).collect();
    paginas.push(vec![0; 0x210]);
    paginas.push(vec![0xFF; 0x210]);
    let eccs = ecc_das_paginas(&paginas.concat());
    let (mut bate, mut recusa) = (0, 0);
    for (mut p, ecc) in paginas.iter().cloned().zip(eccs) {
        por_ecc(&mut p, ecc);
        bate += usize::from(extrair_kv::ecc_confere(&p));
        let bit = r.u64() as usize % (0x200 * 8);
        p[bit / 8] ^= 1 << (bit % 8);
        recusa += usize::from(!extrair_kv::ecc_confere(&p));
    }
    assert_eq!(bate, paginas.len(), "ECC igual ao da referência da Free60");
    assert_eq!(recusa, paginas.len(), "recusa página com 1 bit trocado");
}

// --------------------------------------------------------- formatos e tamanhos

fn conferir_formato(conteudo: &[u8], tamanho: u64, tipo: &str, ecc: bool) {
    let p = Pasta::nova("formato");
    cpukey_txt(&p);
    arquivo(&p.join("nand.bin"), conteudo, tamanho);
    let s = rodar(&p, &["nand.bin", "cpukey.txt"]);
    let ok = s.codigo == 0 && s.texto.contains(tipo) && s.texto.contains("confere (HMAC") && kv_igual(&p.join("KV.bin"));
    assert!(ok && s.segundos < 10.0, "{} ({:.2} s)", s.texto, s.segundos);
    if ecc {
        assert!(s.texto.contains("32 de 32 páginas do KV conferem"), "{}", s.texto);
    }
}

#[test]
fn emmc_de_48_mb_corona_4gb_winchester() {
    conferir_formato(logica(), 48 << 20, "eMMC de 4 GB, cópia de 48 MB", false);
}

#[test]
fn emmc_imagem_inteira_de_4_gb() {
    conferir_formato(logica(), 4 << 30, "eMMC de 4 GB, imagem inteira", false);
}

#[test]
fn nand_de_16_mb_sem_ecc() {
    conferir_formato(logica(), 16 << 20, "imagem de 16 MB, sem ECC", false);
}

#[test]
fn copia_cortada_com_tamanho_quebrado() {
    conferir_formato(logica(), (3 << 20) + 123, "sem ECC", false);
}

fn formato_com_ecc(tamanho: u64, tipo: &str) {
    if ecc_ref().is_none() {
        return pular("NANDs com ECC (sem compilador C)");
    }
    conferir_formato(&com_ecc(logica()), tamanho, tipo, true);
}

#[test]
fn nand_de_16_mb_com_ecc_trinity_corona_16mb() {
    formato_com_ecc(0x1080000, "NAND de 16 MB, com ECC");
}

#[test]
fn nand_de_64_mb_com_ecc() {
    formato_com_ecc(0x4200000, "NAND de 64 MB, com ECC");
}

#[test]
fn nand_de_256_mb_com_ecc_big_block() {
    formato_com_ecc(0x10800000, "NAND de 256 MB, com ECC");
}

#[test]
fn nand_de_512_mb_com_ecc_big_block() {
    formato_com_ecc(0x21000000, "NAND de 512 MB, com ECC");
}

// ----------------------------------------------------- zip, pasta e CPU key

#[test]
fn zip_com_flashdmp_cpukey_e_uma_foto() {
    let p = Pasta::nova("zip");
    let mut foto = vec![0xFF, 0xD8, 0xFF, 0xE0];
    foto.extend(Aleatorio(7).bytes(1 << 20));
    ZipNovo::default()
        .juntar("flashdmp.bin", &emmc_48(logica()))
        .juntar("cpukey.txt", format!("{}\n", hex(&principal().cpu).to_uppercase()).as_bytes())
        .juntar("foto.jpg", &foto)
        .gravar(&p.join("backup.zip"));
    let s = rodar(&p, &["backup.zip"]);
    assert!(s.codigo == 0 && s.texto.contains("flashdmp.bin") && kv_igual(&p.join("KV.bin")), "{}", s.texto);
}

#[test]
fn zip_do_simple_360_nand_flasher_com_a_cpu_key_so_no_log() {
    let p = Pasta::nova("snf");
    let mut updflash = logica()[..0x200].to_vec();
    updflash.resize(0x200 + 0x10000, 0);
    let log = format!(
        "Simple 360 NAND Flasher by Swizzy v1.4b (BETA)\n * Detected MMC NAND device!\nDVD Key: {}\nYour CPUKey is: {}\n",
        hex(&dvd_key()).to_uppercase(),
        hex(&principal().cpu).to_uppercase()
    );
    ZipNovo::default()
        .juntar("recovery.bin", &emmc_48(logica()))
        .juntar("updflash.bin", &updflash)
        .juntar("Simple 360 NAND Flasher.log", log.as_bytes())
        .gravar(&p.join("snf.zip"));
    let s = rodar(&p, &["snf.zip"]);
    assert!(s.codigo == 0 && s.texto.contains("recovery.bin") && kv_igual(&p.join("KV.bin")), "{}", s.texto);
}

#[test]
fn cpu_key_com_espacos_ao_lado_de_um_hash_maior() {
    let p = Pasta::nova("espacos");
    let cpu = hex(&principal().cpu).to_uppercase();
    let espacada: Vec<&str> = (0..4).map(|i| &cpu[i * 8..i * 8 + 8]).collect();
    let info = format!("Hash: {}abcdef12\nCPU Key: {}\n", hex(&outra_cpu()), espacada.join(" "));
    ZipNovo::default()
        .juntar("nanddump1.bin", &emmc_48(logica()))
        .juntar("info.txt", info.as_bytes())
        .gravar(&p.join("espacos.zip"));
    let s = rodar(&p, &["espacos.zip"]);
    assert!(s.codigo == 0 && kv_igual(&p.join("KV.bin")), "{}", s.texto);
}

#[test]
fn pasta_com_a_nand_numa_subpasta() {
    let p = Pasta::nova("pasta");
    arquivo(&p.join("pasta/sub/nanddump.bin"), logica(), 48 << 20);
    texto(&p.join("pasta/cpukey.txt"), &hex(&principal().cpu));
    let s = rodar(&p, &["pasta"]);
    assert!(s.codigo == 0 && kv_igual(&p.join("KV.bin")), "{}", s.texto);
}

#[test]
fn so_a_nand_acha_o_cpukey_na_mesma_pasta() {
    let p = Pasta::nova("so-nand");
    cpukey_txt(&p);
    arquivo(&p.join("flashdmp.bin"), logica(), 48 << 20);
    let s = rodar(&p, &["flashdmp.bin"]);
    assert!(s.codigo == 0 && kv_igual(&p.join("KV.bin")), "{}", s.texto);
}

#[test]
fn sem_cpu_key_e_sem_terminal_erro_claro_e_nada_gravado() {
    let p = Pasta::nova("sozinha");
    arquivo(&p.join("sozinha/flashdmp.bin"), logica(), 48 << 20);
    let s = rodar(&p, &["sozinha/flashdmp.bin"]);
    assert!(s.codigo == 1 && s.texto.contains("não achei a CPU key") && !p.join("KV.bin").exists(), "{}", s.texto);
}

#[cfg(unix)]
#[test]
fn pede_a_cpu_key_num_terminal_sem_mostrar_o_que_foi_digitado() {
    let p = Pasta::nova("digitada");
    arquivo(&p.join("sozinha/flashdmp.bin"), logica(), 48 << 20);
    let cpu = hex(&principal().cpu).to_uppercase();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_extrair-kv"));
    cmd.arg("sozinha/flashdmp.bin").current_dir(&p.0);
    let (tela, ok) = terminal::rodar(cmd, &[("CPU key (", &cpu)]);
    assert!(ok && kv_igual(&p.join("KV.bin")), "{tela}");
    assert!(!tela.to_lowercase().contains(&cpu.to_lowercase()), "a CPU key apareceu na tela: {tela}");
}

// ------------------------------------------------------- erros e proteções

#[test]
fn cpu_key_de_outro_console_nao_grava_nada() {
    let p = Pasta::nova("errada");
    arquivo(&p.join("flashdmp.bin"), logica(), 48 << 20);
    texto(&p.join("errada.txt"), &hex(&outra_cpu()));
    let s = rodar(&p, &["flashdmp.bin", "errada.txt"]);
    assert!(s.codigo == 1 && s.texto.contains("não abriu") && !p.join("KV.bin").exists(), "{}", s.texto);
}

#[test]
fn nao_pega_32_caracteres_do_meio_de_um_hash_maior() {
    let p = Pasta::nova("parecida");
    arquivo(&p.join("flashdmp.bin"), logica(), 48 << 20);
    texto(&p.join("parecida.txt"), &format!("{}abcdef12", hex(&principal().cpu)));
    let s = rodar(&p, &["flashdmp.bin", "parecida.txt"]);
    assert!(s.codigo == 1 && !p.join("KV.bin").exists(), "{}", s.texto);
}

#[test]
fn kv_danificado_avisa_e_nao_grava() {
    let p = Pasta::nova("danificado");
    cpukey_txt(&p);
    arquivo(&p.join("danificada.bin"), &principal().imagem(0x4000, 0x4000, Some(0x3000)), 48 << 20);
    let s = rodar(&p, &["danificada.bin", "cpukey.txt"]);
    assert!(s.codigo == 1 && s.texto.contains("danificado") && !p.join("KV.bin").exists(), "{}", s.texto);
}

#[test]
fn kv_danificado_com_gravar_danificado_grava_e_avisa() {
    let p = Pasta::nova("danificado-gravar");
    cpukey_txt(&p);
    arquivo(&p.join("danificada.bin"), &principal().imagem(0x4000, 0x4000, Some(0x3000)), 48 << 20);
    let s = rodar(&p, &["danificada.bin", "cpukey.txt", "--gravar-danificado"]);
    assert!(s.codigo == 0 && s.texto.contains("danificado") && p.join("KV.bin").exists(), "{}", s.texto);
}

#[test]
fn um_bit_trocado_numa_pagina_com_ecc_diz_quantas_paginas_estao_erradas() {
    if ecc_ref().is_none() {
        return pular("sem compilador C para a referência do ECC");
    }
    let p = Pasta::nova("ecc-ruim");
    cpukey_txt(&p);
    let mut ruim = com_ecc(logica());
    ruim[(0x4000 / 0x200 + 5) * 0x210 + 7] ^= 0x20;
    arquivo(&p.join("raw_ruim.bin"), &ruim, 0x1080000);
    let s = rodar(&p, &["raw_ruim.bin", "cpukey.txt"]);
    assert!(s.codigo == 1 && s.texto.contains("1 página(s) do KV estão com o ECC errado"), "{}", s.texto);
}

#[test]
fn cabecalho_com_posicao_absurda_usa_0x4000() {
    let p = Pasta::nova("cab-ruim");
    cpukey_txt(&p);
    arquivo(&p.join("cab_ruim.bin"), &principal().imagem(0x4000, 0xFFFF_FFFF, None), 48 << 20);
    let s = rodar(&p, &["cab_ruim.bin", "cpukey.txt"]);
    assert!(s.codigo == 0 && s.texto.contains("padrão") && kv_igual(&p.join("KV.bin")), "{}", s.texto);
}

#[test]
fn kv_em_outra_posicao_segue_o_cabecalho() {
    let p = Pasta::nova("kv8000");
    cpukey_txt(&p);
    arquivo(&p.join("kv8000.bin"), &principal().imagem(0x8000, 0x8000, None), 48 << 20);
    let s = rodar(&p, &["kv8000.bin", "cpukey.txt"]);
    let ok = s.codigo == 0 && s.texto.contains("posição 0x8000, lida do cabeçalho") && kv_igual(&p.join("KV.bin"));
    assert!(ok, "{}", s.texto);
}

#[test]
fn ja_existe_kv_bin_nao_mexe_nele() {
    let p = Pasta::nova("ja-existe");
    cpukey_txt(&p);
    arquivo(&p.join("flashdmp.bin"), logica(), 48 << 20);
    fs::write(p.join("KV.bin"), b"antigo").unwrap();
    let s = rodar(&p, &["flashdmp.bin", "cpukey.txt"]);
    let ok = s.codigo == 1 && s.texto.contains("já existe") && fs::read(p.join("KV.bin")).unwrap() == b"antigo";
    assert!(ok, "{}", s.texto);
}

#[test]
fn saida_console_kv_bin_cria_a_pasta_com_permissao_600() {
    let p = Pasta::nova("console");
    cpukey_txt(&p);
    arquivo(&p.join("flashdmp.bin"), logica(), 48 << 20);
    let s = rodar(&p, &["flashdmp.bin", "cpukey.txt", "-o", "console/kv.bin"]);
    assert!(s.codigo == 0 && kv_igual(&p.join("console/kv.bin")), "{}", s.texto);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let modo = fs::metadata(p.join("console/kv.bin")).unwrap().permissions().mode() & 0o777;
        assert_eq!(modo, 0o600, "{}", s.texto);
        let pasta = fs::metadata(p.join("console")).unwrap().permissions().mode() & 0o777;
        assert_eq!(pasta & 0o077, 0, "a pasta console ficou aberta para os outros: {pasta:o}");
    }
}

#[test]
fn saida_com_uma_pasta_grava_kv_bin_dentro() {
    let p = Pasta::nova("saida-pasta");
    cpukey_txt(&p);
    arquivo(&p.join("flashdmp.bin"), logica(), 48 << 20);
    fs::create_dir(p.join("saida")).unwrap();
    let s = rodar(&p, &["flashdmp.bin", "cpukey.txt", "-o", "saida"]);
    assert!(s.codigo == 0 && kv_igual(&p.join("saida/KV.bin")), "{}", s.texto);
}

#[test]
fn arquivo_que_nao_existe_erro_claro() {
    let p = Pasta::nova("nao-existe");
    cpukey_txt(&p);
    let s = rodar(&p, &["nao-existe.bin", "cpukey.txt"]);
    assert!(s.codigo == 1 && s.texto.contains("não achei") && !s.texto.contains("panicked"), "{}", s.texto);
}

#[test]
fn copia_pequena_demais_erro_claro() {
    let p = Pasta::nova("pequena");
    cpukey_txt(&p);
    arquivo(&p.join("pequeno.bin"), &logica()[..0x5000], 0x5000);
    let s = rodar(&p, &["pequeno.bin", "cpukey.txt"]);
    assert!(s.codigo == 1 && s.texto.contains("pequena demais"), "{}", s.texto);
}

#[test]
fn zip_estragado_erro_claro() {
    let p = Pasta::nova("zip-falso");
    cpukey_txt(&p);
    let mut falso = b"PK\x03\x04".to_vec();
    falso.extend(Aleatorio(9).bytes(100));
    fs::write(p.join("falso.zip"), falso).unwrap();
    let s = rodar(&p, &["falso.zip", "cpukey.txt"]);
    assert!(s.codigo == 1 && s.texto.contains("não é um zip válido"), "{}", s.texto);
}

// ------------------------------------------------- além dos testes do Python

/// Zips feitos pelo zip do Info-ZIP (também com zip64), pelo 7-Zip e pelo
/// zipfile do Python (também com "data descriptor"), quando estão instalados.
#[test]
fn zips_feitos_por_outros_programas() {
    let p = Pasta::nova("outros-zips");
    arquivo(&p.join("flashdmp.bin"), logica(), 48 << 20);
    cpukey_txt(&p);
    let python = "import sys, zipfile\n\
        with zipfile.ZipFile('pydeflate.zip', 'w', zipfile.ZIP_DEFLATED) as z:\n\
        \x20   z.write('flashdmp.bin', 'nanddump.bin'); z.write('cpukey.txt')\n\
        with zipfile.ZipFile('pystored.zip', 'w', zipfile.ZIP_STORED) as z:\n\
        \x20   z.write('flashdmp.bin'); z.write('cpukey.txt')\n\
        with zipfile.ZipFile('pyforce64.zip', 'w', zipfile.ZIP_DEFLATED) as z:\n\
        \x20   z.writestr('cpukey.txt', open('cpukey.txt').read())\n\
        \x20   with z.open('flashdmp.bin', 'w', force_zip64=True) as f: f.write(open('flashdmp.bin', 'rb').read())\n\
        class SoEscrita:\n\
        \x20   def __init__(s, f): s.f = f\n\
        \x20   def write(s, b): return s.f.write(b)\n\
        \x20   def flush(s): s.f.flush()\n\
        with open('pystream.zip', 'wb') as saida:\n\
        \x20   with zipfile.ZipFile(SoEscrita(saida), 'w', zipfile.ZIP_DEFLATED) as z:\n\
        \x20       z.write('flashdmp.bin'); z.write('cpukey.txt')\n";
    let feitos = [
        ("zip -q -X", Command::new("zip").args(["-q", "-X", "infozip.zip", "flashdmp.bin", "cpukey.txt"]).current_dir(&p.0).status()),
        ("zip -0", Command::new("zip").args(["-q", "-0", "infozip0.zip", "flashdmp.bin", "cpukey.txt"]).current_dir(&p.0).status()),
        ("zip -fz", Command::new("zip").args(["-q", "-fz", "infozip64.zip", "flashdmp.bin", "cpukey.txt"]).current_dir(&p.0).status()),
        ("7z", Command::new("7z").args(["a", "-tzip", "-bd", "seven.zip", "flashdmp.bin", "cpukey.txt"]).current_dir(&p.0).stdout(Stdio::null()).status()),
        ("python3", Command::new("python3").args(["-c", python]).current_dir(&p.0).status()),
    ];
    let mut zips = Vec::new();
    for (nome, status) in feitos {
        if status.is_ok_and(|s| s.success()) {
            zips.push(nome);
        } else {
            pular(nome);
        }
    }
    let mut lidos = 0;
    for zip in ["infozip.zip", "infozip0.zip", "infozip64.zip", "seven.zip", "pydeflate.zip", "pystored.zip", "pyforce64.zip", "pystream.zip"] {
        if !p.join(zip).exists() {
            continue;
        }
        let _ = fs::remove_file(p.join("KV.bin"));
        let s = rodar(&p, &[zip]);
        assert!(s.codigo == 0 && kv_igual(&p.join("KV.bin")), "{zip}: {}", s.texto);
        lidos += 1;
    }
    eprintln!("  {lidos} zips lidos ({})", zips.join(", "));
}

/// Um zip com outra coisa antes (um .exe que se extrai sozinho), sem as
/// posições corrigidas: o zipfile do Python lê, e este também.
#[test]
fn zip_com_dados_antes_como_um_exe_que_se_extrai_sozinho() {
    let p = Pasta::nova("zip-sfx");
    ZipNovo::default()
        .juntar("flashdmp.bin", &emmc_48(logica()))
        .juntar("cpukey.txt", hex(&principal().cpu).as_bytes())
        .gravar(&p.join("normal.zip"));
    let mut sfx = b"MZ".to_vec();
    sfx.extend(Aleatorio(11).bytes(5000));
    sfx.extend(fs::read(p.join("normal.zip")).unwrap());
    fs::write(p.join("sfx.exe"), sfx).unwrap();
    let s = rodar(&p, &["sfx.exe"]);
    assert!(s.codigo == 0 && kv_igual(&p.join("KV.bin")), "{}", s.texto);
}

#[test]
fn zip_com_senha_de_verdade() {
    let p = Pasta::nova("zip-senha");
    arquivo(&p.join("flashdmp.bin"), logica(), 48 << 20);
    cpukey_txt(&p);
    let feito = Command::new("zip").args(["-q", "-P", "senha", "senha.zip", "flashdmp.bin"]).current_dir(&p.0).status();
    if !feito.is_ok_and(|s| s.success()) {
        return pular("sem o zip do Info-ZIP");
    }
    let s = rodar(&p, &["senha.zip", "cpukey.txt"]);
    assert!(s.codigo == 1 && s.texto.contains("o zip tem senha") && !p.join("KV.bin").exists(), "{}", s.texto);
}

#[test]
fn zip_com_compressao_que_o_programa_nao_le() {
    let p = Pasta::nova("zip-deflate64");
    arquivo(&p.join("flashdmp.bin"), logica(), 48 << 20);
    cpukey_txt(&p);
    let feito = Command::new("7z")
        .args(["a", "-tzip", "-mm=Deflate64", "-bd", "d64.zip", "flashdmp.bin"])
        .current_dir(&p.0)
        .stdout(Stdio::null())
        .status();
    if !feito.is_ok_and(|s| s.success()) {
        return pular("sem o 7-Zip");
    }
    let s = rodar(&p, &["d64.zip", "cpukey.txt"]);
    assert!(s.codigo == 1 && s.texto.contains("tipo de compressão") && s.texto.contains("Extraia"), "{}", s.texto);
}

/// Um zip cortado em qualquer ponto, ou com bytes trocados, vira erro: nunca
/// trava, nunca lê além do arquivo.
#[test]
fn zip_estragado_em_qualquer_ponto_nunca_quebra() {
    let p = Pasta::nova("zip-fuzz");
    let mut nand = logica().to_vec();
    nand.resize(0x40000, 0);
    ZipNovo::default()
        .juntar("flashdmp.bin", &nand)
        .juntar("cpukey.txt", hex(&principal().cpu).as_bytes())
        .gravar(&p.join("bom.zip"));
    let bom = fs::read(p.join("bom.zip")).unwrap();
    let ruim = p.join("ruim.zip");
    let mut r = Aleatorio(0xF0221);
    let ler_tudo = |caminho: &Path| {
        if let Ok(arquivos) = extrair_kv::do_zip(caminho) {
            for a in &arquivos {
                let _ = a.ler(extrair_kv::LEITURA);
            }
            let _ = extrair_kv::escolher_nand(&arquivos);
        }
    };
    for corte in (0..bom.len()).step_by(97).chain(bom.len() - 200..bom.len()) {
        fs::write(&ruim, &bom[..corte]).unwrap();
        ler_tudo(&ruim);
    }
    for _ in 0..3000 {
        let mut v = bom.clone();
        for _ in 0..1 + r.u64() % 4 {
            // de preferência no começo e no fim, onde ficam os cabeçalhos
            let i = match r.u64() % 3 {
                0 => r.u64() as usize % 200,
                1 => v.len() - 1 - r.u64() as usize % 200,
                _ => r.u64() as usize % v.len(),
            };
            v[i] = r.u64() as u8;
        }
        fs::write(&ruim, &v).unwrap();
        ler_tudo(&ruim);
    }
    // e o programa de fora: erro com mensagem, código 1
    for corte in [10, 100, bom.len() / 2, bom.len() - 30, bom.len() - 1] {
        fs::write(&ruim, &bom[..corte]).unwrap();
        let s = rodar(&p, &["ruim.zip"]);
        assert_eq!(s.codigo, 1, "corte em {corte}: {}", s.texto);
        assert!(s.texto.starts_with("ERRO: "), "corte em {corte}: {}", s.texto);
    }
}

#[test]
fn argumentos_ajuda_versao_e_erros() {
    let p = Pasta::nova("argumentos");
    let s = rodar(&p, &["--version"]);
    assert!(s.codigo == 0 && s.texto.starts_with("extrair-kv "), "{}", s.texto);
    let s = rodar(&p, &["--help"]);
    assert!(s.codigo == 0 && s.texto.contains("--gravar-danificado"), "{}", s.texto);
    let s = rodar(&p, &["--nada"]);
    assert!(s.codigo == 2 && s.texto.contains("opção desconhecida"), "{}", s.texto);
    let s = rodar(&p, &["a", "b", "c"]);
    assert!(s.codigo == 2 && s.texto.contains("argumentos a mais"), "{}", s.texto);
    // sem argumentos e sem terminal: mostra a ajuda em vez de abrir o menu
    let s = rodar(&p, &[]);
    assert!(s.codigo == 2 && s.texto.contains("Uso:"), "{}", s.texto);
}

// ------------------------------------------------------------------- menu

/// Roda o programa num terminal de verdade (pty), como uma pessoa usaria.
#[cfg(unix)]
mod terminal {
    use std::ffi::OsStr;
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Write};
    use std::os::fd::BorrowedFd;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use rustix::pty::{grantpt, openpt, ptsname, unlockpt, OpenptFlags};

    /// `passos`: (texto que aparece na tela, resposta digitada). Devolve a tela
    /// e se todos os passos aconteceram.
    pub fn rodar(mut cmd: Command, passos: &[(&str, &str)]) -> (String, bool) {
        let mestre = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).unwrap();
        grantpt(&mestre).unwrap();
        unlockpt(&mestre).unwrap();
        let nome = ptsname(&mestre, Vec::new()).unwrap();
        let escravo = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(rustix::fs::OFlags::NOCTTY.bits() as i32)
            .open(OsStr::from_bytes(nome.as_bytes()))
            .unwrap();
        cmd.stdin(Stdio::from(escravo.try_clone().unwrap()))
            .stdout(Stdio::from(escravo.try_clone().unwrap()))
            .stderr(Stdio::from(escravo));
        // SAFETY: entre o fork e o exec só há duas chamadas de sistema
        // (setsid e o ioctl que faz do pty o terminal do programa).
        #[allow(unsafe_code)]
        unsafe {
            cmd.pre_exec(|| {
                rustix::process::setsid()?;
                rustix::process::ioctl_tiocsctty(BorrowedFd::borrow_raw(0))?;
                Ok(())
            });
        }
        let mut filho = cmd.spawn().unwrap();
        drop(cmd); // fecha o lado do programa neste processo
        let mut escrita = File::from(mestre.try_clone().unwrap());
        let mut leitura = File::from(mestre);
        let (tx, rx) = mpsc::channel();
        let leitor = std::thread::spawn(move || {
            let mut buf = [0u8; 65536];
            while let Ok(n @ 1..) = leitura.read(&mut buf) {
                if tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        let mut bruto = Vec::new();
        let (mut pos, mut i) = (0, 0);
        let fim = Instant::now() + Duration::from_secs(60);
        loop {
            let tela = match std::str::from_utf8(&bruto) {
                Ok(t) => t,
                Err(e) => std::str::from_utf8(&bruto[..e.valid_up_to()]).unwrap(),
            };
            if let Some((esperado, resposta)) = passos.get(i) {
                if let Some(achou) = tela[pos..].find(esperado) {
                    pos += achou + esperado.len();
                    escrita.write_all(format!("{resposta}\r").as_bytes()).unwrap();
                    i += 1;
                }
            }
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(pedaco) => bruto.extend(pedaco),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if Instant::now() > fim {
                let _ = filho.kill();
                break;
            }
        }
        let _ = filho.wait();
        drop(escrita);
        let _ = leitor.join();
        (String::from_utf8_lossy(&bruto).replace('\r', ""), i == passos.len())
    }
}

/// A casa de teste do menu: dois consoles, cópias com nomes quaisquer, uma
/// foto, o editor e uma cópia longe dos lugares de costume.
#[cfg(unix)]
struct CasaDeTeste {
    pasta: Pasta,
    casa: PathBuf,
    programa: PathBuf,
    a: Console,
    b: Console,
    editor: PathBuf,
    pendrive: PathBuf,
}

#[cfg(unix)]
impl CasaDeTeste {
    fn nova() -> CasaDeTeste {
        let pasta = Pasta::nova("menu");
        let casa = pasta.join("casa");
        // o programa fica numa pasta só dele: a pasta do programa também é um lugar de busca
        let programa = pasta.join("programa/extrair-kv");
        fs::create_dir_all(programa.parent().unwrap()).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_extrair-kv"), &programa).unwrap();
        let a = console("111111111111", "X111111-001", 1);
        let b = console("222222222222", "X222222-001", 2);
        let imagem = |c: &Console| {
            let mut img = vec![0u8; 0x10000];
            img[..2].copy_from_slice(&[0xFF, 0x4F]);
            img[0x60..0x64].copy_from_slice(&0x4000u32.to_be_bytes());
            img[0x6C..0x70].copy_from_slice(&0x4000u32.to_be_bytes());
            img[0x4000..0x8000].copy_from_slice(&c.cifrado());
            img
        };
        let downloads = casa.join("Downloads");
        let editor = downloads.join("RE5 Editor");
        let pendrive = casa.join("Desktop/pendrive/Simple 360 NAND Flasher");
        for p in [&editor, &pendrive, &casa.join("Documents/copia sem chave"), &casa.join("saida"), &casa.join("longe/daqui")] {
            fs::create_dir_all(p).unwrap();
        }
        fs::write(editor.join("biohazard5-save-editor"), b"\x7fELF").unwrap();
        let mut foto = vec![0xFF, 0xD8, 0xFF, 0xE0];
        foto.extend(Aleatorio(5).bytes(1 << 16));
        fs::write(downloads.join("foto.jpg"), foto).unwrap();
        // console A: zip e NAND com nomes quaisquer, CPU key num texto qualquer
        let mut emmc_a = imagem(&a);
        emmc_a.resize(48 << 20, 0);
        ZipNovo::default()
            .juntar("dump_final_ok.bin", &emmc_a)
            .juntar("anotacoes.txt", format!("cpu key: {}\n", hex(&a.cpu).to_uppercase()).as_bytes())
            .gravar(&downloads.join("meu xbox da sala (backup velho).zip"));
        // console B: a pasta do Simple 360 NAND Flasher no pendrive, e uma cópia sem a CPU key
        arquivo(&pendrive.join("flashdmp.bin"), &imagem(&b), 48 << 20);
        texto(&pendrive.join("cpukey.txt"), &hex(&b.cpu).to_uppercase());
        arquivo(&casa.join("Documents/copia sem chave/nand do quarto.img"), &imagem(&b), 48 << 20);
        // e uma longe dos lugares de costume
        arquivo(&casa.join("longe/daqui/x.bin"), &imagem(&a), 48 << 20);
        texto(&casa.join("longe/daqui/cpukey.txt"), &hex(&a.cpu));
        CasaDeTeste { pasta, casa, programa, a, b, editor, pendrive }
    }

    fn menu(&self, passos: &[(&str, &str)]) -> (String, bool) {
        self.menu_em(&self.casa, passos)
    }

    fn menu_em(&self, casa: &Path, passos: &[(&str, &str)]) -> (String, bool) {
        let mut cmd = Command::new(&self.programa);
        cmd.current_dir(casa).env("HOME", casa).env("USER", "usuario-de-teste").env("NO_COLOR", "1");
        let (tela, ok) = terminal::rodar(cmd, passos);
        for c in [&self.a, &self.b] {
            assert!(!tela.to_lowercase().contains(&hex(&c.cpu)), "uma CPU key apareceu na tela: {tela}");
        }
        (tela, ok)
    }
}

#[cfg(unix)]
fn ler(caminho: &Path) -> Option<Vec<u8>> {
    fs::read(caminho).ok()
}

#[cfg(unix)]
#[test]
fn menu_acha_as_copias_pelo_conteudo_e_grava_no_editor() {
    let t = CasaDeTeste::nova();
    let (tela, ok) = t.menu(&[("Escolha a cópia: ", "1"), ("Escolha: ", "1"), ("ENTER para sair", "")]);
    let lista = tela.split("Escolha a cópia:").next().unwrap();
    assert!(ok, "{tela}");
    // acha as 3 cópias pelo conteúdo, com nomes quaisquer, e não a foto
    for nome in ["meu xbox da sala (backup velho).zip", "flashdmp.bin", "nand do quarto.img"] {
        assert!(lista.contains(nome), "{nome}: {tela}");
    }
    assert!(!lista.contains("foto.jpg"), "{tela}");
    // mostra a série e a peça de cada console
    assert!(lista.contains("série 111111111111 · peça X111111-001"), "{tela}");
    assert!(lista.contains("série 222222222222 · peça X222222-001"), "{tela}");
    // avisa qual cópia está sem a CPU key
    assert!(lista.contains("sem a CPU key junto"), "{tela}");
    // acha o editor e grava em console/kv.bin
    assert_eq!(ler(&t.editor.join("console/kv.bin")), Some(t.a.kv.clone()), "{tela}");
}

#[cfg(unix)]
#[test]
fn menu_grava_ao_lado_da_copia_com_a_serie_no_nome() {
    let t = CasaDeTeste::nova();
    let (tela, ok) = t.menu(&[("Escolha a cópia: ", "2"), ("Escolha: ", "2"), ("ENTER para sair", "")]);
    assert!(ok && ler(&t.pendrive.join("KV-222222222222.bin")) == Some(t.b.kv.clone()), "{tela}");
}

#[cfg(unix)]
#[test]
fn menu_pede_a_cpu_key_que_falta_recusa_a_de_outro_console_e_aceita_a_certa() {
    let t = CasaDeTeste::nova();
    let saida = format!("'{}'", t.casa.join("saida").display());
    let (tela, ok) = t.menu(&[
        ("Escolha a cópia: ", "3"),
        ("Digite a CPU key", &hex(&t.a.cpu)),
        ("Digite a CPU key", &hex(&t.b.cpu).to_uppercase()),
        ("Escolha: ", "o"),
        ("Pasta: ", &saida),
        ("ENTER para sair", ""),
    ]);
    assert!(ok && tela.contains("não abre o KV desta cópia"), "{tela}");
    assert_eq!(ler(&t.casa.join("saida/KV-222222222222.bin")), Some(t.b.kv.clone()), "{tela}");
}

#[cfg(unix)]
#[test]
fn menu_nao_grava_por_cima_de_um_kv_bin_que_ja_existe() {
    let t = CasaDeTeste::nova();
    fs::create_dir_all(t.editor.join("console")).unwrap();
    fs::write(t.editor.join("console/kv.bin"), b"antigo").unwrap();
    let (tela, ok) = t.menu(&[
        ("Escolha a cópia: ", "1"),
        ("Escolha: ", "1"),
        ("Escolha: ", "v"),
        ("Escolha a cópia: ", "s"),
        ("ENTER para sair", ""),
    ]);
    assert!(ok && tela.contains("já existe"), "{tela}");
    assert_eq!(ler(&t.editor.join("console/kv.bin")).as_deref(), Some(&b"antigo"[..]), "{tela}");
}

#[cfg(unix)]
#[test]
fn menu_caminho_digitado_entre_aspas_e_a_pasta_do_editor_digitada() {
    let t = CasaDeTeste::nova();
    let copia = format!("\"{}\"", t.casa.join("longe/daqui/x.bin").display());
    let longe = t.casa.join("longe").display().to_string();
    let (tela, ok) = t.menu(&[
        ("Escolha a cópia: ", "c"),
        ("Caminho da cópia: ", &copia),
        ("Escolha: ", "e"),
        ("Pasta do editor: ", &longe),
        ("ENTER para sair", ""),
    ]);
    assert!(ok && ler(&t.casa.join("longe/console/kv.bin")) == Some(t.a.kv.clone()), "{tela}");
}

#[cfg(unix)]
#[test]
fn menu_sem_nenhuma_copia_diz_onde_procurou_e_oferece_digitar_o_caminho() {
    let t = CasaDeTeste::nova();
    let vazia = t.pasta.join("vazia");
    fs::create_dir_all(&vazia).unwrap();
    let (tela, ok) = t.menu_em(&vazia, &[("Escolha a cópia: ", "s"), ("ENTER para sair", "")]);
    assert!(ok && tela.contains("Não achei nenhuma cópia") && !tela.contains("panicked"), "{tela}");
}

#[cfg(unix)]
#[test]
fn menu_ctrl_c_na_cpu_key_cancela_e_devolve_o_terminal() {
    let t = CasaDeTeste::nova();
    let (tela, ok) = t.menu(&[("Escolha a cópia: ", "3"), ("Digite a CPU key", "\x03"), ("ENTER para sair", "")]);
    assert!(ok && tela.contains("Cancelado. Nada foi gravado."), "{tela}");
}
