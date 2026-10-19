//! A NAND do Xbox 360: páginas, ECC, posição do keyvault, decifrar e conferir.
use std::ops::Range;

use crate::cripto::{hmac_sha1, iguais, rc4};
use crate::texto::limpo;

/// Dados de uma página da NAND.
pub const PAGINA: usize = 0x200;
/// A mesma página com os 16 bytes de spare (ECC).
pub const PAGINA_ECC: usize = 0x210;
/// O keyvault tem sempre 16 KB.
pub const KV_TAMANHO: usize = 0x4000;
/// E fica nesta posição em todos os consoles conhecidos.
pub const KV_PADRAO: usize = 0x4000;
/// Posição máxima aceita no cabeçalho.
const KV_MAXIMO: usize = 0x100000;
/// Basta ler o começo da cópia, nunca o arquivo todo.
pub const LEITURA: usize = 0x120000;
/// Arquivos de texto maiores que isso não têm CPU key.
pub const TEXTO_MAXIMO: usize = 64 * 1024;
/// Cabeçalho + keyvault.
pub const NAND_MINIMA: u64 = 0x8000;
/// CPU keys tentadas no máximo.
pub const CHAVES_MAXIMO: usize = 256;
/// Começo de toda NAND de console comum.
pub const MAGIC: [u8; 2] = [0xFF, 0x4F];

/// O console acrescenta estes 2 bytes no HMAC do KV.
const APENDICE_HMAC: [u8; 2] = [0x07, 0x12];

// Posições dentro do keyvault decifrado (libxenon, kvlookup)
const SERIE: Range<usize> = 0xB0..0xBC;
const CHAVE_PRIVADA: usize = 0x298;
const CERTIFICADO: usize = 0x9C8;
const PECA: Range<usize> = 0x9CF..0x9DA;

/// A CPU key: 16 bytes, única de cada console.
pub type CpuKey = [u8; 16];

/// O keyvault achado numa cópia da NAND.
#[derive(Clone, Debug)]
pub struct Achado {
    /// A cópia tem os 16 bytes de spare (ECC) em cada página.
    pub com_ecc: bool,
    /// Posição do keyvault na NAND, sem contar o ECC.
    pub inicio: usize,
    /// A posição veio do cabeçalho da NAND (senão, é a padrão).
    pub do_cabecalho: bool,
    /// Páginas do keyvault com o ECC errado.
    pub ruins: usize,
    /// O keyvault decifrado, 16 KB.
    pub kv: Vec<u8>,
}

/// Decifra o keyvault e confere o HMAC, como o console faz (libxenon, kv_read).
///
/// Os 16 primeiros bytes são o HMAC-SHA1 do resto do KV decifrado, com a CPU
/// key como chave. Deles sai também a chave do RC4 que cifra o resto.
pub fn decifrar(cifrado: &[u8], cpu_key: &CpuKey) -> (Vec<u8>, bool) {
    let mut kv = cifrado.to_vec();
    let Some(cabeca) = cifrado.get(..0x10) else {
        return (kv, false);
    };
    let mut chave_rc4 = [0u8; 16];
    chave_rc4.copy_from_slice(&hmac_sha1(cpu_key, &[cabeca])[..16]);
    rc4(&chave_rc4, &mut kv[0x10..]);
    let esperado = hmac_sha1(cpu_key, &[&kv[0x10..], &APENDICE_HMAC]);
    let confere = iguais(&esperado[..16], cabeca);
    (kv, confere)
}

/// Número grande do XeCrypt: palavras de 8 bytes, da menos para a mais
/// significativa, cada uma em big-endian.
fn numero_xecrypt<const N: usize>(b: &[u8]) -> [u64; N] {
    std::array::from_fn(|i| {
        let mut palavra = [0u8; 8];
        palavra.copy_from_slice(&b[i * 8..i * 8 + 8]);
        u64::from_be_bytes(palavra)
    })
}

/// p × q, de 512 bits cada, num número de 1024 bits.
fn multiplicar(p: &[u64; 8], q: &[u64; 8]) -> [u64; 16] {
    let mut r = [0u64; 16];
    for (i, &a) in p.iter().enumerate() {
        let mut vai_um = 0u128;
        for (j, &b) in q.iter().enumerate() {
            let t = u128::from(a) * u128::from(b) + u128::from(r[i + j]) + vai_um;
            r[i + j] = t as u64;
            vai_um = t >> 64;
        }
        r[i + 8] = vai_um as u64;
    }
    r
}

fn maior_que_um(x: &[u64]) -> bool {
    x[0] > 1 || x[1..].iter().any(|&palavra| palavra != 0)
}

/// Confere a série, o certificado e a chave privada do console (p × q = n).
pub fn estrutura_confere(kv: &[u8]) -> bool {
    let Some(k) = kv.get(CHAVE_PRIVADA..CHAVE_PRIVADA + 0x1D0) else {
        return false;
    };
    let Some(serie) = kv.get(SERIE) else {
        return false;
    };
    let n = numero_xecrypt::<16>(&k[0x10..0x90]);
    let p = numero_xecrypt::<8>(&k[0x90..0xD0]);
    let q = numero_xecrypt::<8>(&k[0xD0..0x110]);
    serie.iter().all(u8::is_ascii_digit)
        && kv.get(CERTIFICADO..CERTIFICADO + 2) == Some(&[0x01, 0xA8][..])
        && maior_que_um(&p)
        && maior_que_um(&q)
        && multiplicar(&p, &q) == n
}

/// Confere os 26 bits de ECC de uma página (512 bytes de dados + 16 de spare).
///
/// Algoritmo da Free60 (NAND File System): os 0x1066 primeiros bits da página,
/// em palavras de 32 bits little-endian invertidas, passam por um LFSR.
pub fn ecc_confere(pagina: &[u8]) -> bool {
    if pagina.len() < PAGINA_ECC {
        return false;
    }
    let mut val = 0u32;
    let mut v = 0u32;
    for i in 0..0x1066usize {
        if i & 31 == 0 {
            let p = i >> 3;
            v = !u32::from_le_bytes([pagina[p], pagina[p + 1], pagina[p + 2], pagina[p + 3]]);
        }
        val ^= v & 1;
        v >>= 1;
        if val & 1 != 0 {
            val ^= 0x6954559;
        }
        val >>= 1;
    }
    let val = !val;
    let s = &pagina[PAGINA..PAGINA_ECC];
    s[0xC] & 0xC0 == ((val << 6) & 0xC0) as u8
        && s[0xD] == (val >> 2) as u8
        && s[0xE] == (val >> 10) as u8
        && s[0xF] == (val >> 18) as u8
}

/// Lê da imagem lógica (sem ECC). Devolve (dados, páginas com ECC errado).
pub fn ler_logico(prefixo: &[u8], com_ecc: bool, inicio: usize, tamanho: usize) -> (Vec<u8>, usize) {
    if !com_ecc {
        let fim = inicio.saturating_add(tamanho).min(prefixo.len());
        return (prefixo[inicio.min(fim)..fim].to_vec(), 0);
    }
    let mut dados = Vec::with_capacity(tamanho + PAGINA);
    let mut ruins = 0;
    for pagina in inicio / PAGINA..inicio.saturating_add(tamanho).div_ceil(PAGINA) {
        let Some(bloco) = pagina.checked_mul(PAGINA_ECC).and_then(|comeco| prefixo.get(comeco..comeco.checked_add(PAGINA_ECC)?))
        else {
            break;
        };
        if !ecc_confere(bloco) {
            ruins += 1;
        }
        dados.extend_from_slice(&bloco[..PAGINA]);
    }
    let resto = inicio % PAGINA;
    let fim = resto.saturating_add(tamanho).min(dados.len());
    (dados[resto.min(fim)..fim].to_vec(), ruins)
}

/// Inteiro big-endian com os bytes que existirem no intervalo (como o
/// int.from_bytes de um pedaço cortado).
fn be_parcial(b: &[u8], intervalo: Range<usize>) -> u64 {
    let fim = intervalo.end.min(b.len());
    b[intervalo.start.min(fim)..fim].iter().fold(0, |n, &x| n << 8 | u64::from(x))
}

/// Posição do keyvault no cabeçalho da NAND (0x60: tamanho, 0x6C: posição).
pub fn posicao_do_kv(cabecalho: &[u8]) -> (usize, bool) {
    let tamanho = be_parcial(cabecalho, 0x60..0x64);
    let inicio = be_parcial(cabecalho, 0x6C..0x70);
    if tamanho == KV_TAMANHO as u64 && inicio.is_multiple_of(PAGINA as u64) && (PAGINA as u64..=KV_MAXIMO as u64).contains(&inicio) {
        return (inicio as usize, true);
    }
    (KV_PADRAO, false)
}

/// Procura o keyvault na cópia e tenta cada CPU key.
///
/// Devolve (achado, danificado): o keyvault que abriu com o HMAC certo, e o
/// primeiro que não abriu, mas tem a estrutura de um keyvault de verdade (a
/// CPU key é deste console, mas a cópia está danificada).
pub fn abrir_kv(prefixo: &[u8], tamanho: u64, chaves: &[CpuKey]) -> (Option<Achado>, Option<Achado>) {
    let com_ecc_primeiro = tamanho.is_multiple_of(PAGINA_ECC as u64);
    let mut danificado = None;
    for com_ecc in [com_ecc_primeiro, !com_ecc_primeiro] {
        let (cabecalho, _) = ler_logico(prefixo, com_ecc, 0, PAGINA);
        let (inicio, do_cabecalho) = posicao_do_kv(&cabecalho);
        let mut posicoes = vec![(inicio, do_cabecalho)];
        if inicio != KV_PADRAO {
            posicoes.push((KV_PADRAO, false));
        }
        for (inicio, do_cabecalho) in posicoes {
            let (cifrado, ruins) = ler_logico(prefixo, com_ecc, inicio, KV_TAMANHO);
            if cifrado.len() < KV_TAMANHO {
                continue;
            }
            for chave in chaves {
                let (kv, confere) = decifrar(&cifrado, chave);
                let info = || Achado { com_ecc, inicio, do_cabecalho, ruins, kv: kv.clone() };
                if confere {
                    return (Some(info()), danificado);
                }
                if danificado.is_none() && estrutura_confere(&kv) {
                    danificado = Some(info());
                }
            }
        }
    }
    (None, danificado)
}

/// O tipo da cópia, pelo tamanho.
pub fn descrever(tamanho: u64, com_ecc: bool) -> String {
    if com_ecc {
        let mb = tamanho / PAGINA_ECC as u64 * PAGINA as u64 / (1 << 20);
        let dica = match mb {
            16 => " (Xenon a Jasper, Trinity, Corona 16 MB)",
            64 => " (NAND de 64 MB, ou o começo de uma big block)",
            256 | 512 => " (Jasper big block)",
            _ => "",
        };
        return format!("NAND de {mb} MB, com ECC{dica}");
    }
    let mb = tamanho as f64 / (1u64 << 20) as f64;
    if mb == 48.0 {
        return "eMMC de 4 GB, cópia de 48 MB (Corona 4 GB, Winchester)".into();
    }
    if mb >= 4000.0 {
        return "eMMC de 4 GB, imagem inteira (Corona 4 GB, Winchester)".into();
    }
    format!("imagem de {mb:.0} MB, sem ECC")
}

/// O número de série e o número da peça (part number) do console.
pub fn serie_e_peca(kv: &[u8]) -> (String, String) {
    let serie: String =
        kv.get(SERIE).unwrap_or_default().iter().map(|&b| if b.is_ascii() { char::from(b) } else { '\u{FFFD}' }).collect();
    let peca = kv.get(PECA).unwrap_or_default();
    let peca = if !peca.is_empty() && peca.iter().all(|b| (32..127).contains(b)) {
        String::from_utf8_lossy(peca).into_owned()
    } else {
        String::new()
    };
    (limpo(&serie), peca)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn multiplicar_com_vai_um() {
        let max = [u64::MAX; 8];
        let r = multiplicar(&max, &max);
        // (2^512 - 1)^2 = 2^1024 - 2^513 + 1
        let mut esperado = [u64::MAX; 16];
        esperado[0] = 1;
        esperado[8] = u64::MAX - 1;
        for palavra in &mut esperado[1..8] {
            *palavra = 0;
        }
        assert_eq!(r, esperado);
        let mut dois = [0u64; 8];
        dois[0] = 2;
        let mut tres = [0u64; 8];
        tres[0] = 3;
        assert_eq!(multiplicar(&dois, &tres)[0], 6);
    }

    #[test]
    fn posicao_do_kv_aceita_so_valores_sensatos() {
        let mut cab = vec![0u8; 0x200];
        cab[0x60..0x64].copy_from_slice(&0x4000u32.to_be_bytes());
        cab[0x6C..0x70].copy_from_slice(&0x8000u32.to_be_bytes());
        assert_eq!(posicao_do_kv(&cab), (0x8000, true));
        cab[0x6C..0x70].copy_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
        assert_eq!(posicao_do_kv(&cab), (KV_PADRAO, false));
        cab[0x6C..0x70].copy_from_slice(&0x8100u32.to_be_bytes());
        assert_eq!(posicao_do_kv(&cab), (KV_PADRAO, false));
        assert_eq!(posicao_do_kv(&[]), (KV_PADRAO, false));
    }

    #[test]
    fn descrever_como_o_python() {
        assert_eq!(descrever(48 << 20, false), "eMMC de 4 GB, cópia de 48 MB (Corona 4 GB, Winchester)");
        assert_eq!(descrever(4 << 30, false), "eMMC de 4 GB, imagem inteira (Corona 4 GB, Winchester)");
        assert_eq!(descrever((3 << 20) + 123, false), "imagem de 3 MB, sem ECC");
        assert_eq!(descrever(0x1080000, true), "NAND de 16 MB, com ECC (Xenon a Jasper, Trinity, Corona 16 MB)");
        assert_eq!(descrever(0x21000000, true), "NAND de 512 MB, com ECC (Jasper big block)");
        assert_eq!(descrever(0x1080000, false), "imagem de 16 MB, sem ECC");
    }

    #[test]
    fn ler_logico_corta_como_fatia_do_python() {
        let dados: Vec<u8> = (0..100u8).collect();
        assert_eq!(ler_logico(&dados, false, 90, 20).0, &dados[90..]);
        assert!(ler_logico(&dados, false, 200, 20).0.is_empty());
        assert!(ler_logico(&dados, true, 0, 0x200).0.is_empty());
    }

    #[test]
    fn nada_quebra_com_dados_curtos() {
        assert!(!estrutura_confere(&[0u8; 10]));
        assert!(!ecc_confere(&[0u8; 10]));
        assert_eq!(serie_e_peca(&[0u8; 4]), (String::new(), String::new()));
        let (_, confere) = decifrar(&[1, 2, 3], &[0; 16]);
        assert!(!confere);
        assert_eq!(abrir_kv(&[0xFF; 0x100], 0x100, &[[0; 16]]).0.map(|a| a.inicio), None);
    }
}
