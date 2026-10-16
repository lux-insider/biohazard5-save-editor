//! Save do Resident Evil 5: Xbox 360 e PC (Steam).
//!
//! Os dois guardam o mesmo `savedata.bin` interno. O checksum soma as palavras
//! de [0x10, 0x3BAC) e de [0x3BB0, fim): no PC o fim é 0x52B0; no Xbox é
//! 0x4BC0 (o valor da palavra em 0x4 do savedata.bin, 0x6F0 antes do PC, como
//! os outros endereços). Conferido em saves gravados pelo próprio console: com
//! 0x52B0 o checksum só batia quando o trecho [0x4BC0, 0x52B0) era todo zero, e
//! um save editado com esse fim deu "corrompido" no jogo. As diferenças:
//!
//! * Xbox 360: o arquivo é um pacote STFS "CON" com o savedata.bin dentro,
//!   sem criptografia, big-endian. Ao gravar: checksum, hashes SHA-1 do
//!   pacote e a assinatura RSA (com o keyvault do console ou a chave interna).
//! * PC: o próprio arquivo é o savedata.bin, little-endian, cifrado com XOR
//!   encadeado em blocos de 8 bytes.
//!
//! Os campos do PC ficam 0xB4 bytes adiante dos do Xbox (dinheiro 0x194 no
//! PC e 0xE0 no Xbox); as roupas, filtros, munição infinita, arquivos e
//! figuras seguem o mesmo deslocamento — conferido no save do Xbox, em que
//! 0x7C tem exatamente 19 bits (as 19 armas) e 0x80 tem 12 (os 12 arquivos).

use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::fs;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, String>;

const TAM_MINIMO: usize = 0x52B0;
const OFF_CHECKSUM: usize = 0x08;
const OFF_DATA: usize = 0x10;

// ---------------------------------------------------------------- listas

/// Listas de desbloqueio (bit N = item N a partir do fim da lista).
pub struct Lista {
    pub chave: &'static str,
    pub titulo: &'static str,
    /// deslocamento a partir do início do bloco (Xbox 0x70 / PC 0x124)
    pub rel: usize,
    /// 64 bits (duas palavras: baixa, depois alta)
    pub dupla: bool,
    pub nomes: &'static [&'static str],
}

pub const LISTAS: &[Lista] = &[
    Lista { chave: "chris_roupas", titulo: "Roupas do Chris", rel: 0x00, dupla: false,
        nomes: &["Warrior", "S.T.A.R.S.", "Safari", "BSAA"] },
    Lista { chave: "sheva_roupas", titulo: "Roupas da Sheva", rel: 0x04, dupla: false,
        nomes: &["Business", "Tribal", "Clubbin'", "BSAA"] },
    Lista { chave: "filtros", titulo: "Filtros de tela", rel: 0x08, dupla: false,
        nomes: &["Ruído", "Retrô", "Clássico", "Padrão"] },
    Lista { chave: "municao", titulo: "Munição infinita", rel: 0x0C, dupla: false,
        nomes: &[
            "RPG", "S&W M500", "L. Hawk", "S&W M29", "H&K PSG-1", "Dragunov SVD", "S75",
            "SIG 556", "H&K MP5", "AK-74", "VZ61", "Hydra", "Jail Breaker", "M3", "Ithaca M37",
            "M93R", "SIG P226", "H&K P8", "M92F",
        ] },
    Lista { chave: "arquivos", titulo: "Arquivos (Biblioteca)", rel: 0x10, dupla: false,
        nomes: &[
            "Albert Wesker", "Excella Gionne", "Jill Valentine", "Tricell", "U-8",
            "Tribo Ndipaya", "Ricardo Irving", "Sheva Alomar", "Chris Redfield", "Majini",
            "BSAA", "A Saga de Resident Evil",
        ] },
    Lista { chave: "figuras", titulo: "Figuras", rel: 0x14, dupla: true,
        nomes: &[
            "Jill (rara)", "Sheva (rara)", "Chris (rara)", "Irving (transformado)", "Ndesu",
            "Popokarimu", "U-8", "Uroboros Aheri", "Crocodilo", "Adjule", "Bui Kichwa",
            "Kipepeo", "Licker β", "Uroboros", "Majini de moto", "Majini da metralhadora",
            "Majini da motosserra", "Executioner Majini", "Big Man Majini", "Reaper",
            "Majini (Duvalia)", "Majini (Base B)", "Majini (Base A)", "Majini gigante",
            "Majini (Pântano C)", "Majini (Pântano B)", "Majini (Pântano A)", "Majini (Agitador)",
            "Majini (Cephalo)", "Majini (Vila D)", "Majini (Vila C)", "Majini (Vila B)",
            "Majini (Vila A)", "Reynard", "Kirk", "Dave", "DeChant", "Spencer", "Irving",
            "Excella", "Wesker", "Jill (controlada)", "Josh", "Sheva (BSAA)", "Chris (BSAA)",
        ] },
];

// ---------------------------------------------------------------- utilitários

fn sha1(d: &[u8]) -> [u8; 20] {
    Sha1::digest(d).into()
}

fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes(d[o..o + 4].try_into().unwrap())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

fn de_hex(s: &str, tam: usize, nome: &str) -> Result<Vec<u8>> {
    let s = s.trim();
    if s.len() != tam * 2 || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("{nome} precisa ter {} dígitos hexadecimais", tam * 2));
    }
    Ok((0..tam).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap()).collect())
}

/// Números do XeCrypt ficam em qwords na ordem inversa.
fn qwrev(b: &[u8]) -> Vec<u8> {
    b.chunks(8).rev().flatten().copied().collect()
}

fn fixo_be(n: &BigUint, len: usize) -> Vec<u8> {
    let b = n.to_bytes_be();
    let mut out = vec![0u8; len.saturating_sub(b.len())];
    out.extend_from_slice(&b);
    out
}

// ---------------------------------------------------------------- savedata.bin (comum)

/// O savedata.bin decifrado, com a ordem dos bytes e os endereços de cada
/// plataforma.
pub struct Interno {
    pub d: Vec<u8>,
    pub big_endian: bool,
    off_gold: usize,
    off_listas: usize,
    /// Baú: slots de 12 bytes, id e quantidade em u16.
    off_inv: usize,
    /// Chris e Sheva: slots de 0x2C bytes, id e quantidade em u32.
    off_chris: usize,
    off_sheva: usize,
    inv_slots: usize,
    /// Fim da segunda região do checksum (Xbox 0x4BC0, PC 0x52B0).
    fim_checksum: usize,
}

impl Interno {
    fn xbox(d: Vec<u8>) -> Self {
        Self {
            d, big_endian: true, off_gold: 0xE0, off_listas: 0x70,
            off_inv: 0x3158, off_chris: 0x3690, off_sheva: 0x3AB0, inv_slots: 84, fim_checksum: 0x4BC0,
        }
    }
    /// No PC o baú fica 0x6F4 depois do Xbox e os personagens 0x6F0 —
    /// conferido num save real da Steam contra a tela do jogo (inventário
    /// do Chris, da Sheva e o baú, slot por slot). Os slots 74 em diante,
    /// no PC, guardam outros dados: o baú para no 73.
    fn pc(d: Vec<u8>) -> Self {
        Self {
            d, big_endian: false, off_gold: 0x194, off_listas: 0x124,
            off_inv: 0x384C, off_chris: 0x3D80, off_sheva: 0x41A0, inv_slots: 74, fim_checksum: 0x52B0,
        }
    }

    fn u16(&self, o: usize) -> u32 {
        let b = [self.d[o], self.d[o + 1]];
        (if self.big_endian { u16::from_be_bytes(b) } else { u16::from_le_bytes(b) }) as u32
    }

    fn set16(&mut self, o: usize, v: u16) {
        let b = if self.big_endian { v.to_be_bytes() } else { v.to_le_bytes() };
        self.d[o..o + 2].copy_from_slice(&b);
    }

    fn inventario(&self) -> Vec<Slot> {
        (0..self.inv_slots)
            .map(|i| {
                let o = self.off_inv + i * 12;
                Slot { slot: i, id: self.u16(o), amount: self.u16(o + 2) }
            })
            .collect()
    }

    fn personagem(&self, base: usize) -> Vec<Slot> {
        (0..PERS_VISIVEIS)
            .map(|i| {
                let o = base + i * SLOT_PERS;
                Slot { slot: i, id: self.u32(o), amount: self.u32(o + 4) }
            })
            .collect()
    }

    fn aplica_inventario(&mut self, m: &Mudancas) -> Result<()> {
        for s in &m.inventario {
            if s.slot >= self.inv_slots || s.id > 0xFFFF || s.amount > 0xFFFF {
                return Err("slot do inventário inválido".into());
            }
            let o = self.off_inv + s.slot * 12;
            self.set16(o, s.id as u16);
            self.set16(o + 2, s.amount as u16);
        }
        for (slots, base) in [(&m.chris, self.off_chris), (&m.sheva, self.off_sheva)] {
            for s in slots {
                if s.slot >= PERS_VISIVEIS {
                    return Err("slot de personagem inválido".into());
                }
                let o = base + s.slot * SLOT_PERS;
                self.set32(o, s.id);
                self.set32(o + 4, s.amount);
            }
        }
        Ok(())
    }

    pub fn u32(&self, o: usize) -> u32 {
        let b: [u8; 4] = self.d[o..o + 4].try_into().unwrap();
        if self.big_endian { u32::from_be_bytes(b) } else { u32::from_le_bytes(b) }
    }

    fn set32(&mut self, o: usize, v: u32) {
        let b = if self.big_endian { v.to_be_bytes() } else { v.to_le_bytes() };
        self.d[o..o + 4].copy_from_slice(&b);
    }

    pub fn checksum(&self) -> u32 {
        let a = (0x10..0x3BAC).step_by(4).map(|o| self.u32(o));
        let b = (0x3BB0..self.fim_checksum).step_by(4).map(|o| self.u32(o));
        a.chain(b).fold(0u32, u32::wrapping_add)
    }

    pub fn checksum_salvo(&self) -> u32 {
        self.u32(OFF_CHECKSUM)
    }

    fn fecha_checksum(&mut self) {
        let c = self.checksum();
        self.set32(OFF_CHECKSUM, c);
    }

    fn bits(&self, l: &Lista) -> u64 {
        let o = self.off_listas + l.rel;
        let baixo = self.u32(o) as u64;
        if l.dupla { baixo | (self.u32(o + 4) as u64) << 32 } else { baixo }
    }

    fn set_bits(&mut self, l: &Lista, v: u64) {
        let o = self.off_listas + l.rel;
        self.set32(o, v as u32);
        if l.dupla {
            self.set32(o + 4, (v >> 32) as u32);
        }
    }

    /// bit do item `i` da lista (o último da lista é o bit 0)
    fn bit_de(l: &Lista, i: usize) -> u64 {
        1u64 << (l.nomes.len() - 1 - i)
    }

    fn listas_json(&self) -> Value {
        LISTAS
            .iter()
            .map(|l| {
                let v = self.bits(l);
                let marcados: Vec<bool> = (0..l.nomes.len()).map(|i| v & Self::bit_de(l, i) != 0).collect();
                json!({ "chave": l.chave, "titulo": l.titulo, "opcoes": l.nomes, "marcados": marcados })
            })
            .collect()
    }

    fn comum_json(&self) -> Value {
        let data: Vec<u32> = (0..6).map(|i| self.u32(OFF_DATA + i * 4)).collect();
        json!({
            "data": data,
            "gold": self.u32(self.off_gold),
            "points": self.u32(self.off_gold + 4),
            "checksum_salvo": format!("{:08X}", self.checksum_salvo()),
            "checksum_real": format!("{:08X}", self.checksum()),
            "checksum_ok": self.checksum_salvo() == self.checksum(),
            "listas": self.listas_json(),
        })
    }

    fn aplica_comum(&mut self, m: &Mudancas) -> Result<()> {
        if let Some(g) = m.gold {
            self.set32(self.off_gold, g);
        }
        if let Some(p) = m.points {
            self.set32(self.off_gold + 4, p);
        }
        if let Some(dt) = &m.data {
            let [ano, mes, dia, h, min, s] = *dt;
            if !(2000..=2099).contains(&ano) || !(1..=12).contains(&mes) || !(1..=31).contains(&dia) || h > 23 || min > 59 || s > 59 {
                return Err("data do save inválida".into());
            }
            for (i, v) in dt.iter().enumerate() {
                self.set32(OFF_DATA + i * 4, *v);
            }
        }
        for (chave, marcados) in &m.listas {
            let l = LISTAS.iter().find(|l| l.chave == chave).ok_or(format!("lista desconhecida: {chave}"))?;
            if marcados.len() != l.nomes.len() {
                return Err(format!("{}: quantidade de opções errada", l.titulo));
            }
            // só mexe nos bits conhecidos; os outros ficam como estavam
            let mut v = self.bits(l);
            for (i, &on) in marcados.iter().enumerate() {
                let b = Self::bit_de(l, i);
                if on { v |= b } else { v &= !b }
            }
            self.set_bits(l, v);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- mudanças vindas da tela

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Slot {
    pub slot: usize,
    pub id: u32,
    pub amount: u32,
}

#[derive(Deserialize, Default, Debug)]
pub struct Mudancas {
    pub gold: Option<u32>,
    pub points: Option<u32>,
    pub data: Option<[u32; 6]>,
    #[serde(default)]
    pub listas: Vec<(String, Vec<bool>)>,
    pub steam_id: Option<String>,
    pub profile_id: Option<String>,
    pub device_id: Option<String>,
    /// Nome do save no dashboard (só Xbox).
    #[serde(default)]
    pub nome_save: Option<String>,
    #[serde(default)]
    pub inventario: Vec<Slot>,
    #[serde(default)]
    pub chris: Vec<Slot>,
    #[serde(default)]
    pub sheva: Vec<Slot>,
}

// ---------------------------------------------------------------- PC

const XOR_INICIAL: [u8; 8] = [0x00, 0x21, 0x11, 0x08, 0xC0, 0x4B, 0x00, 0x00];

fn pc_decifra(cifrado: &[u8]) -> Vec<u8> {
    let mut chave = XOR_INICIAL.to_vec();
    let mut out = Vec::with_capacity(cifrado.len());
    for bloco in cifrado.chunks(8) {
        out.extend(bloco.iter().zip(&chave).map(|(a, b)| a ^ b));
        chave = bloco.to_vec();
    }
    out
}

fn pc_cifra(claro: &[u8]) -> Vec<u8> {
    let mut chave = XOR_INICIAL.to_vec();
    let mut out = Vec::with_capacity(claro.len());
    for bloco in claro.chunks(8) {
        let c: Vec<u8> = bloco.iter().zip(&chave).map(|(a, b)| a ^ b).collect();
        out.extend_from_slice(&c);
        chave = c;
    }
    out
}

// ---------------------------------------------------------------- Xbox 360: STFS e assinatura

const PKG_PERFIL: (usize, usize) = (0x371, 8);
const PKG_DEVICE: (usize, usize) = (0x3FD, 0x14);
const PKG_CONSOLE: (usize, usize) = (0x36C, 5);
const VD: usize = 0x379;
const NOME_INTERNO: &[u8] = b"savedata.bin";

/// Title ID do Resident Evil 5 e o tipo "save de jogo" no cabeçalho do pacote.
pub const TITLE_ID_RE5: u32 = 0x434307D4;
const TIPO_SAVE: u32 = 1;
const OFF_TIPO: usize = 0x344;
const OFF_TITLE_ID: usize = 0x360;

// Cabeçalho do pacote (metadados), conferido nos saves reais do Xbox:
// nomes e textos em UTF-16BE de 0x100 bytes; miniaturas em PNG.
const OFF_VERSAO_META: usize = 0x348;
const OFF_NOMES: usize = 0x411; // 9 idiomas
/// Versão 2 do cabeçalho: mais 3 idiomas, depois da miniatura do save.
const OFF_NOMES_EXTRA: usize = 0x541A;
/// Cada nome tem 0x100 bytes (128 caracteres UTF-16).
const TAM_NOME: usize = 0x100;
const OFF_PUBLISHER: usize = 0x1611;
const OFF_JOGO: usize = 0x1691;
const OFF_TAM_MINI: usize = 0x1712;
const OFF_TAM_MINI_JOGO: usize = 0x1716;
const OFF_MINI: usize = 0x171A;
/// Na versão 2 do cabeçalho há 3 nomes a mais antes, mas a miniatura do
/// jogo cai no mesmo lugar nas duas versões (0x3D00 + 0x300 = 0x4000).
const OFF_MINI_JOGO: usize = 0x571A;
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

const SLOT_PERS: usize = 0x2C;
const PERS_VISIVEIS: usize = 9;

const DIGEST_INFO: [u8; 15] = [0x30, 0x21, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x0e, 0x03, 0x02, 0x1a, 0x05, 0x00, 0x04, 0x14];

pub fn e_pacote_stfs(d: &[u8]) -> bool {
    d.len() >= 0x1000 && matches!(&d[..4], b"CON " | b"LIVE" | b"PIRS")
}

/// Chave RSA e certificado do console, lidos do keyvault decifrado (kv.bin).
pub struct ChaveConsole {
    cert: Vec<u8>,
    n: BigUint,
    d: BigUint,
    pub console: String,
}

impl ChaveConsole {
    pub fn carrega(p: &Path) -> Result<Self> {
        Self::de_bytes(&fs::read(p).map_err(|e| format!("keyvault: {e}"))?)
    }

    /// Lê o keyvault decifrado já na memória (por exemplo, embutido no programa).
    pub fn de_bytes(kv: &[u8]) -> Result<Self> {
        if kv.len() < 0x4000 {
            return Err("keyvault inválido".into());
        }
        let cert = kv[0x9C8..0x9C8 + 0x1A8].to_vec();
        let k = &kv[0x298..0x298 + 0x1D0];
        let e = BigUint::from(be32(k, 4));
        let n = BigUint::from_bytes_be(&qwrev(&k[0x10..0x90]));
        let p_ = BigUint::from_bytes_be(&qwrev(&k[0x90..0xD0]));
        let q = BigUint::from_bytes_be(&qwrev(&k[0xD0..0x110]));
        if &p_ * &q != n || u16::from_be_bytes([cert[0], cert[1]]) != 0x1A8 {
            return Err("keyvault inválido".into());
        }
        let um = BigUint::from(1u32);
        let d = e.modinv(&((&p_ - &um) * (&q - &um))).ok_or("keyvault inválido")?;
        let console = String::from_utf8_lossy(&cert[7..0x12]).into_owned();
        Ok(Self { cert, n, d, console })
    }

    /// Chave montada a partir das partes já separadas (certificado, n, p e q
    /// em big-endian comum), como a chave interna do programa. Confere
    /// p*q == n e que o módulo do certificado é o mesmo da chave.
    #[allow(dead_code)] // não usada por examples/verifica.rs
    pub fn de_componentes(cert: &[u8], n: &[u8], p: &[u8], q: &[u8]) -> Result<Self> {
        if cert.len() != 0x1A8 || u16::from_be_bytes([cert[0], cert[1]]) != 0x1A8 {
            return Err("certificado inválido".into());
        }
        let n = BigUint::from_bytes_be(n);
        let (p_, q) = (BigUint::from_bytes_be(p), BigUint::from_bytes_be(q));
        if &p_ * &q != n || BigUint::from_bytes_be(&qwrev(&cert[0x28..0xA8])) != n {
            return Err("chave inválida".into());
        }
        let e = BigUint::from(be32(cert, 0x24));
        let um = BigUint::from(1u32);
        let d = e.modinv(&((&p_ - &um) * (&q - &um))).ok_or("chave inválida")?;
        let console = String::from_utf8_lossy(&cert[7..0x12]).into_owned();
        Ok(Self { cert: cert.to_vec(), n, d, console })
    }

    fn assina(&self, dados: &[u8]) -> Vec<u8> {
        let m = bloco_pkcs1(dados);
        let mut s = fixo_be(&BigUint::from_bytes_be(&m).modpow(&self.d, &self.n), 0x80);
        s.reverse();
        s
    }
}

/// Bloco PKCS#1 v1.5 de 0x80 bytes com o SHA-1 dos dados:
/// 00 01 FF..FF 00 DigestInfo hash.
fn bloco_pkcs1(dados: &[u8]) -> Vec<u8> {
    let mut m = vec![0x00, 0x01];
    m.extend(std::iter::repeat(0xFF).take(0x80 - 3 - DIGEST_INFO.len() - 20));
    m.push(0x00);
    m.extend_from_slice(&DIGEST_INFO);
    m.extend_from_slice(&sha1(dados));
    m
}

/// Confere a assinatura do pacote com a chave pública do próprio certificado.
pub fn assinatura_ok(pkg: &[u8]) -> bool {
    let cert = &pkg[4..4 + 0x1A8];
    let e = BigUint::from(be32(cert, 0x24));
    let n = BigUint::from_bytes_be(&qwrev(&cert[0x28..0xA8]));
    if n == BigUint::from(0u32) {
        return false;
    }
    let mut sig = pkg[0x1AC..0x22C].to_vec();
    sig.reverse();
    let m = fixo_be(&BigUint::from_bytes_be(&sig).modpow(&e, &n), 0x80);
    // o bloco inteiro tem que bater, não só o hash no fim
    m == bloco_pkcs1(&pkg[0x22C..0x344])
}

pub fn ids_do_pacote(pkg: &[u8]) -> (String, String) {
    let (po, pl) = PKG_PERFIL;
    let (dof, dl) = PKG_DEVICE;
    (hex(&pkg[po..po + pl]), hex(&pkg[dof..dof + dl]))
}

/// O que o cabeçalho do pacote diz sobre o save (aba Save).
pub struct Metadados {
    /// Nome do save no dashboard (o primeiro idioma preenchido).
    pub nome: String,
    pub jogo: String,
    pub publisher: String,
    pub title_id: u32,
    pub console_id: String,
    /// Miniaturas em PNG (do save e do jogo), se houver.
    pub miniatura: Option<Vec<u8>>,
    pub miniatura_jogo: Option<Vec<u8>>,
}

fn utf16_fixo(d: &[u8], o: usize) -> String {
    let Some(b) = d.get(o..o + 0x100) else { return String::new() };
    let u: Vec<u16> = b.chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).take_while(|&c| c != 0).collect();
    String::from_utf16_lossy(&u).trim().to_string()
}

fn png_em(d: &[u8], o_tam: usize, o: usize, max: usize) -> Option<Vec<u8>> {
    let tam = u32::from_be_bytes(d.get(o_tam..o_tam + 4)?.try_into().ok()?) as usize;
    if tam == 0 || tam > max {
        return None;
    }
    let b = d.get(o..o + tam)?;
    b.starts_with(PNG).then(|| b.to_vec())
}

/// Onde fica o nome de cada idioma (9, ou 12 na versão 2 do cabeçalho).
fn offsets_nomes(pkg: &[u8]) -> Vec<usize> {
    let versao = pkg.get(OFF_VERSAO_META..OFF_VERSAO_META + 4).map(|b| be32(b, 0)).unwrap_or(0);
    let mut v: Vec<usize> = (0..9).map(|i| OFF_NOMES + i * TAM_NOME).collect();
    if versao >= 2 {
        v.extend((0..3).map(|i| OFF_NOMES_EXTRA + i * TAM_NOME));
    }
    v
}

/// Grava o nome do save no dashboard. Vai só para os idiomas que já têm
/// nome (como o jogo deixou); se nenhum tiver, vai para os 9 principais.
/// Os idiomas vazios continuam vazios.
fn grava_nome(pkg: &mut [u8], nome: &str) -> Result<()> {
    let nome = nome.trim();
    let utf16: Vec<u16> = nome.encode_utf16().collect();
    if utf16.is_empty() || utf16.len() > TAM_NOME / 2 || nome.chars().any(char::is_control) {
        return Err("o nome do save precisa ter de 1 a 128 caracteres, sem caracteres de controle".into());
    }
    let mut campo = vec![0u8; TAM_NOME];
    for (i, c) in utf16.iter().enumerate() {
        campo[i * 2..i * 2 + 2].copy_from_slice(&c.to_be_bytes());
    }
    let todos = offsets_nomes(pkg);
    let preenchidos: Vec<usize> = todos.iter().copied().filter(|&o| !utf16_fixo(pkg, o).is_empty()).collect();
    let alvos = if preenchidos.is_empty() { todos[..9].to_vec() } else { preenchidos };
    for o in alvos {
        pkg.get_mut(o..o + TAM_NOME).ok_or("cabeçalho do pacote truncado")?.copy_from_slice(&campo);
    }
    Ok(())
}

pub fn metadados(pkg: &[u8]) -> Metadados {
    let versao = pkg.get(OFF_VERSAO_META..OFF_VERSAO_META + 4).map(|b| be32(b, 0)).unwrap_or(0);
    let max = if versao >= 2 { 0x3D00 } else { 0x4000 };
    let (co, cl) = PKG_CONSOLE;
    Metadados {
        nome: offsets_nomes(pkg).into_iter().map(|o| utf16_fixo(pkg, o)).find(|s| !s.is_empty()).unwrap_or_default(),
        jogo: utf16_fixo(pkg, OFF_JOGO),
        publisher: utf16_fixo(pkg, OFF_PUBLISHER),
        title_id: be32(pkg, OFF_TITLE_ID),
        console_id: hex(&pkg[co..co + cl]),
        miniatura: png_em(pkg, OFF_TAM_MINI, OFF_MINI, max),
        miniatura_jogo: png_em(pkg, OFF_TAM_MINI_JOGO, OFF_MINI_JOGO, max),
    }
}

/// Pacote STFS com um nível de hash (saves pequenos).
pub struct Stfs {
    pub d: Vec<u8>,
    base: usize,
    shift: u32,
    tabela: usize,
}

impl Stfs {
    pub fn novo(d: Vec<u8>) -> Result<Self> {
        if !e_pacote_stfs(&d) {
            return Err("não é um pacote STFS (CON/LIVE/PIRS)".into());
        }
        // Só pacotes assinados por console: LIVE/PIRS têm outro cabeçalho e
        // sairiam incoerentes ao assinar.
        if &d[..4] != b"CON " {
            return Err("este pacote é LIVE/PIRS; o editor só abre saves assinados por console (CON)".into());
        }
        let title = be32(&d, OFF_TITLE_ID);
        if title != TITLE_ID_RE5 {
            return Err(format!("não é um save do Resident Evil 5 (Title ID {title:08X}, esperado {TITLE_ID_RE5:08X})"));
        }
        let tipo = be32(&d, OFF_TIPO);
        if tipo != TIPO_SAVE {
            return Err(format!("o pacote é do Resident Evil 5, mas não é um save de jogo (tipo {tipo:08X})"));
        }
        Self::abre_con(d)
    }

    /// Abre qualquer pacote CON de um nível de hash, sem exigir que seja save
    /// do RE5 (o perfil, por exemplo). Só leitura de arquivos por nome.
    pub fn abre_con(d: Vec<u8>) -> Result<Self> {
        if !e_pacote_stfs(&d) || &d[..4] != b"CON " {
            return Err("não é um pacote CON do Xbox 360".into());
        }
        let tam_cab = be32(&d, 0x340) as usize;
        let sep = d[VD + 2];
        let base = (tam_cab + 0xFFF) & 0xF000;
        let shift = if base == 0xB000 || sep & 1 == 0 { 1 } else { 0 };
        // a tabela de hash ativa é a segunda cópia quando o bit 1 está ligado
        let tabela = base + if sep & 2 != 0 { 0x1000 } else { 0 };
        if be32(&d, VD + 0x1C) >= 0xAA {
            return Err("pacote grande demais (mais de um nível de hash)".into());
        }
        if d.len() < tabela + 0x1000 {
            return Err("pacote truncado".into());
        }
        Ok(Self { d, base, shift, tabela })
    }

    fn off(&self, b: usize) -> usize {
        self.base + ((b + (((b + 0xAA) / 0xAA) << self.shift)) << 12)
    }

    fn entrada(&self, b: usize) -> usize {
        self.tabela + b * 0x18
    }

    fn proximo(&self, b: usize) -> usize {
        let e = self.entrada(b);
        let x = &self.d[e + 0x15..e + 0x18];
        (x[0] as usize) << 16 | (x[1] as usize) << 8 | x[2] as usize
    }

    /// Com um nível de hash, todo bloco tem entrada na tabela (0..0xAA).
    /// Um número fora disso é arquivo corrompido: erro em vez de ler fora.
    fn bloco(&self, b: usize) -> Result<&[u8]> {
        if b >= 0xAA {
            return Err("bloco fora da tabela de hash (pacote corrompido)".into());
        }
        let o = self.off(b);
        self.d.get(o..o + 0x1000).ok_or_else(|| "bloco fora do pacote".to_string())
    }

    /// Lê um arquivo do pacote pelo nome: o diretório pode ter vários blocos e
    /// o arquivo pode ser contíguo (blocos em sequência) ou encadeado pela
    /// tabela de hash. Usado para o "Account" do perfil.
    #[allow(dead_code)] // não usada por examples/verifica.rs
    pub fn arquivo(&self, nome: &str) -> Result<Vec<u8>> {
        let blocos_dir = u16::from_le_bytes([self.d[VD + 3], self.d[VD + 4]]) as usize;
        let mut b = self.bloco_diretorio();
        for _ in 0..blocos_dir.max(1) {
            for ent in self.bloco(b)?.chunks(0x40) {
                let len = (ent[0x28] & 0x3F) as usize;
                if len > 0 && &ent[..len] == nome.as_bytes() {
                    let contiguo = ent[0x28] & 0x40 != 0;
                    let mut bl = u32::from_le_bytes([ent[0x2F], ent[0x30], ent[0x31], 0]) as usize;
                    let tam = be32(ent, 0x34) as usize;
                    let mut out = Vec::with_capacity(tam);
                    for _ in 0..(tam + 0xFFF) / 0x1000 {
                        out.extend_from_slice(self.bloco(bl)?);
                        bl = if contiguo { bl + 1 } else { self.proximo(bl) };
                    }
                    out.truncate(tam);
                    return Ok(out);
                }
            }
            b = self.proximo(b);
        }
        Err(format!("{nome} não encontrado no pacote"))
    }

    /// Primeiro bloco do diretório (a tabela de arquivos do pacote).
    fn bloco_diretorio(&self) -> usize {
        u32::from_le_bytes([self.d[VD + 5], self.d[VD + 6], self.d[VD + 7], 0]) as usize
    }

    fn blocos_do_arquivo(&self) -> Result<(Vec<usize>, usize)> {
        let ft = self.bloco_diretorio();
        for ent in self.bloco(ft)?.chunks(0x40) {
            let len = (ent[0x28] & 0x3F) as usize;
            if len > 0 && &ent[..len] == NOME_INTERNO {
                let mut b = u32::from_le_bytes([ent[0x2F], ent[0x30], ent[0x31], 0]) as usize;
                let tam = be32(ent, 0x34) as usize;
                let mut blocos = Vec::new();
                for _ in 0..(tam + 0xFFF) / 0x1000 {
                    self.bloco(b)?;
                    blocos.push(b);
                    b = self.proximo(b);
                }
                return Ok((blocos, tam));
            }
        }
        Err("savedata.bin não encontrado no pacote".into())
    }

    pub fn le(&self) -> Result<Vec<u8>> {
        let (blocos, tam) = self.blocos_do_arquivo()?;
        let mut out = Vec::with_capacity(blocos.len() * 0x1000);
        for b in blocos {
            out.extend_from_slice(self.bloco(b)?);
        }
        out.truncate(tam);
        Ok(out)
    }

    pub fn grava(&mut self, conteudo: &[u8]) -> Result<()> {
        let (blocos, tam) = self.blocos_do_arquivo()?;
        if conteudo.len() != tam {
            return Err("o tamanho do arquivo interno não pode mudar".into());
        }
        for (i, &b) in blocos.iter().enumerate() {
            let parte = &conteudo[i * 0x1000..((i + 1) * 0x1000).min(tam)];
            let o = self.off(b);
            self.d[o..o + parte.len()].copy_from_slice(parte);
            let h = sha1(&self.d[o..o + 0x1000]);
            let e = self.entrada(b);
            self.d[e..e + 20].copy_from_slice(&h);
        }
        // O diretório não muda; recalcular só conserta um hash dele que
        // esteja errado (num save íntegro o valor é o mesmo).
        let ft = self.bloco_diretorio();
        let h = sha1(self.bloco(ft)?);
        let e = self.entrada(ft);
        self.d[e..e + 20].copy_from_slice(&h);
        let topo = sha1(&self.d[self.tabela..self.tabela + 0x1000]);
        self.d[VD + 8..VD + 0x1C].copy_from_slice(&topo);
        let cab = sha1(&self.d[0x344..self.base]);
        self.d[0x32C..0x340].copy_from_slice(&cab);
        Ok(())
    }

    fn reassina(&mut self, k: &ChaveConsole) {
        self.d[4..4 + 0x1A8].copy_from_slice(&k.cert);
        let s = k.assina(&self.d[0x22C..0x344]);
        self.d[0x1AC..0x22C].copy_from_slice(&s);
    }

    /// O que não confere nos hashes do pacote; vazio = tudo certo.
    pub fn falhas_hash(&self) -> Vec<&'static str> {
        let confere = |b: usize| {
            let e = self.entrada(b);
            self.bloco(b).map(|x| sha1(x)[..] == self.d[e..e + 20]).unwrap_or(false)
        };
        let mut falhas = Vec::new();
        match self.blocos_do_arquivo() {
            Ok((blocos, _)) if blocos.iter().all(|&b| confere(b)) => {}
            Ok(_) => falhas.push("blocos do savedata.bin"),
            Err(_) => falhas.push("savedata.bin não encontrado"),
        }
        if !confere(self.bloco_diretorio()) {
            falhas.push("bloco do diretório");
        }
        if sha1(&self.d[self.tabela..self.tabela + 0x1000])[..] != self.d[VD + 8..VD + 0x1C] {
            falhas.push("tabela de hash");
        }
        if sha1(&self.d[0x344..self.base])[..] != self.d[0x32C..0x340] {
            falhas.push("cabeçalho (0x32C)");
        }
        falhas
    }

    pub fn hashes_ok(&self) -> bool {
        self.falhas_hash().is_empty()
    }
}

// ---------------------------------------------------------------- o save aberto

pub enum Plataforma {
    Xbox(Stfs),
    Pc,
}

pub struct Save {
    pub caminho: PathBuf,
    pub plat: Plataforma,
    pub interno: Interno,
}

impl Save {
    /// Abre e descobre a plataforma sozinho: pacote CON = Xbox 360; senão
    /// tenta o formato do PC e confere o checksum.
    pub fn abre(caminho: &Path) -> Result<Self> {
        let bruto = fs::read(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
        if e_pacote_stfs(&bruto) {
            let pkg = Stfs::novo(bruto)?;
            let d = pkg.le()?;
            if d.len() < TAM_MINIMO {
                return Err("o savedata.bin do pacote é pequeno demais para ser do RE5".into());
            }
            return Ok(Self { caminho: caminho.into(), plat: Plataforma::Xbox(pkg), interno: Interno::xbox(d) });
        }
        if bruto.len() < TAM_MINIMO {
            return Err("arquivo pequeno demais para ser um save do RE5".into());
        }
        let interno = Interno::pc(pc_decifra(&bruto));
        if interno.checksum_salvo() != interno.checksum() {
            return Err("não reconheci o arquivo: não é um pacote do Xbox 360 e, decifrado como save de PC, o checksum não bate".into());
        }
        Ok(Self { caminho: caminho.into(), plat: Plataforma::Pc, interno })
    }

    pub fn info(&self) -> Value {
        let mut v = self.interno.comum_json();
        v["inventario"] = json!(self.interno.inventario());
        v["chris"] = json!(self.interno.personagem(self.interno.off_chris));
        v["sheva"] = json!(self.interno.personagem(self.interno.off_sheva));
        v["caminho"] = json!(self.caminho.display().to_string());
        match &self.plat {
            Plataforma::Pc => {
                v["plataforma"] = json!("pc");
                v["steam_id"] = json!(u64::from_le_bytes(self.interno.d[0..8].try_into().unwrap()).to_string());
            }
            Plataforma::Xbox(pkg) => {
                let (perfil, device) = ids_do_pacote(&pkg.d);
                v["plataforma"] = json!("xbox");
                v["profile_id"] = json!(perfil);
                v["device_id"] = json!(device);
                v["assinatura_ok"] = json!(assinatura_ok(&pkg.d));
                v["assinado_por"] = json!(String::from_utf8_lossy(&pkg.d[0xB..0x16]));
                v["falhas_hash"] = json!(pkg.falhas_hash());
                let m = metadados(&pkg.d);
                v["nome_save"] = json!(m.nome);
                v["jogo"] = json!(m.jogo);
                v["publisher"] = json!(m.publisher);
                v["title_id"] = json!(format!("{:08X}", m.title_id));
                v["console_id"] = json!(m.console_id);
            }
        }
        v
    }

    /// Miniaturas PNG do pacote (save e jogo); o save de PC não tem.
    #[allow(dead_code)] // não usada por examples/verifica.rs
    pub fn miniaturas(&self) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        match &self.plat {
            Plataforma::Xbox(pkg) => {
                let m = metadados(&pkg.d);
                (m.miniatura, m.miniatura_jogo)
            }
            Plataforma::Pc => (None, None),
        }
    }

    pub fn aplica(&mut self, m: &Mudancas) -> Result<()> {
        self.interno.aplica_comum(m)?;
        match &mut self.plat {
            Plataforma::Pc => {
                if let Some(s) = m.steam_id.as_deref().filter(|s| !s.trim().is_empty()) {
                    let id: u64 = s.trim().parse().map_err(|_| "Steam ID precisa ser um número".to_string())?;
                    self.interno.d[0..8].copy_from_slice(&id.to_le_bytes());
                }
            }
            Plataforma::Xbox(pkg) => {
                for (valor, (o, t), nome) in [
                    (&m.profile_id, PKG_PERFIL, "O perfil (XUID)"),
                    (&m.device_id, PKG_DEVICE, "O device ID"),
                ] {
                    if let Some(v) = valor.as_deref().filter(|v| !v.is_empty()) {
                        pkg.d[o..o + t].copy_from_slice(&de_hex(v, t, nome)?);
                    }
                }
                // Fica no cabeçalho (coberto pelo hash em 0x32C, que o
                // grava() recalcula, e pela assinatura).
                if let Some(n) = &m.nome_save {
                    grava_nome(&mut pkg.d, n)?;
                }
            }
        }
        self.interno.aplica_inventario(m)?;
        Ok(())
    }

    /// Monta o arquivo final. No Xbox, assina se houver keyvault; devolve
    /// os bytes e o console que assinou.
    #[allow(dead_code)] // usada por examples/verifica.rs
    pub fn monta(&mut self, kv: Option<&Path>) -> Result<(Vec<u8>, Option<String>)> {
        // O save de PC não é assinado: a chave só é lida no Xbox, como antes.
        let chave = match self.plat {
            Plataforma::Xbox(_) => kv.map(ChaveConsole::carrega).transpose()?,
            Plataforma::Pc => None,
        };
        self.monta_com(chave.as_ref())
    }

    /// Igual a `monta`, com a chave já carregada.
    pub fn monta_com(&mut self, chave: Option<&ChaveConsole>) -> Result<(Vec<u8>, Option<String>)> {
        self.interno.fecha_checksum();
        match &mut self.plat {
            Plataforma::Pc => Ok((pc_cifra(&self.interno.d), None)),
            Plataforma::Xbox(pkg) => {
                pkg.grava(&self.interno.d)?;
                let mut console = None;
                if let Some(k) = chave {
                    pkg.reassina(k);
                    console = Some(k.console.clone());
                }
                Ok((pkg.d.clone(), console))
            }
        }
    }

    /// Relê um arquivo gravado e confere tudo que dá para conferir.
    pub fn confere(caminho: &Path, assinado: bool) -> bool {
        let Ok(s) = Save::abre(caminho) else { return false };
        let chk = s.interno.checksum_salvo() == s.interno.checksum();
        match &s.plat {
            Plataforma::Pc => chk,
            Plataforma::Xbox(pkg) => chk && pkg.hashes_ok() && (!assinado || assinatura_ok(&pkg.d)),
        }
    }
}

/// Perfil e device ID de outro save do Xbox (para trocar de perfil/console).
pub fn ids_de_outro(caminho: &Path) -> Result<(String, String)> {
    let d = fs::read(caminho).map_err(|e| e.to_string())?;
    if !e_pacote_stfs(&d) {
        return Err("esse arquivo não é um save do Xbox 360 (pacote CON)".into());
    }
    Ok(ids_do_pacote(&d))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn xor_do_pc_ida_e_volta() {
        let claro: Vec<u8> = (0..0x5663u32).map(|i| (i * 7 + 3) as u8).collect();
        assert_eq!(pc_decifra(&pc_cifra(&claro)), claro);
    }

    #[test]
    fn bits_preservam_o_que_nao_e_da_lista() {
        let mut i = Interno::xbox(vec![0; TAM_MINIMO]);
        let fig = &LISTAS[5];
        i.set_bits(fig, 1 << 45 | 1); // bit 45 não está na lista
        let m = Mudancas { listas: vec![("figuras".into(), vec![true; 45])], ..Default::default() };
        i.aplica_comum(&m).unwrap();
        assert_eq!(i.bits(fig), (1 << 46) - 1);
        let m = Mudancas { listas: vec![("figuras".into(), vec![false; 45])], ..Default::default() };
        i.aplica_comum(&m).unwrap();
        assert_eq!(i.bits(fig), 1 << 45);
        assert_eq!(i.u32(0x84), 0);
        assert_eq!(i.u32(0x88), 1 << 13);
    }
}

#[cfg(test)]
mod testes_chave_interna {
    use super::*;
    use crate::sistema::chave_interna as c;

    #[test]
    fn chave_interna_assina_e_confere() {
        let k = ChaveConsole::de_componentes(&c::CERT, &c::N, &c::P, &c::Q).unwrap();
        // pacote mínimo: só o que a assinatura cobre e o certificado
        let mut pkg = vec![0u8; 0x1000];
        pkg[..4].copy_from_slice(b"CON ");
        pkg[0x22C..0x344].iter_mut().enumerate().for_each(|(i, b)| *b = i as u8);
        pkg[4..4 + 0x1A8].copy_from_slice(&k.cert);
        let s = k.assina(&pkg[0x22C..0x344]);
        pkg[0x1AC..0x22C].copy_from_slice(&s);
        assert!(assinatura_ok(&pkg));
        pkg[0x300] ^= 1; // qualquer byte assinado alterado invalida
        assert!(!assinatura_ok(&pkg));
    }

    #[test]
    fn chave_interna_recusa_partes_trocadas() {
        assert!(ChaveConsole::de_componentes(&c::CERT, &c::N, &c::Q, &c::Q).is_err());
    }

    #[test]
    fn assinatura_exige_o_bloco_inteiro() {
        // Assinatura "falsa" cujo bloco termina com o DigestInfo e o hash
        // certos, mas sem o enchimento 00 01 FF..FF 00: tem que ser recusada.
        let k = ChaveConsole::de_componentes(&c::CERT, &c::N, &c::P, &c::Q).unwrap();
        let mut pkg = vec![0u8; 0x1000];
        pkg[..4].copy_from_slice(b"CON ");
        pkg[4..4 + 0x1A8].copy_from_slice(&k.cert);
        let mut m = bloco_pkcs1(&pkg[0x22C..0x344]);
        m[1] = 0x02; // enchimento errado, final igual
        let mut s = fixo_be(&BigUint::from_bytes_be(&m).modpow(&k.d, &k.n), 0x80);
        s.reverse();
        pkg[0x1AC..0x22C].copy_from_slice(&s);
        assert!(!assinatura_ok(&pkg));
    }
}

#[cfg(test)]
mod testes_pacote {
    use super::*;

    /// Pacote mínimo com Title ID e tipo escolhidos (o resto zerado).
    fn pacote(magica: &[u8; 4], title: u32, tipo: u32) -> Vec<u8> {
        let mut d = vec![0u8; 0xC000];
        d[..4].copy_from_slice(magica);
        d[OFF_TIPO..OFF_TIPO + 4].copy_from_slice(&tipo.to_be_bytes());
        d[OFF_TITLE_ID..OFF_TITLE_ID + 4].copy_from_slice(&title.to_be_bytes());
        d
    }

    fn erro(d: Vec<u8>) -> String {
        Stfs::novo(d).err().unwrap_or_default()
    }

    #[test]
    fn recusa_live_e_pirs() {
        assert!(erro(pacote(b"LIVE", TITLE_ID_RE5, 1)).contains("LIVE/PIRS"));
        assert!(erro(pacote(b"PIRS", TITLE_ID_RE5, 1)).contains("LIVE/PIRS"));
    }

    #[test]
    fn recusa_outro_jogo_e_outro_tipo() {
        assert!(erro(pacote(b"CON ", 0x4D5307E6, 1)).contains("não é um save do Resident Evil 5"));
        assert!(erro(pacote(b"CON ", TITLE_ID_RE5, 0x2)).contains("não é um save de jogo"));
    }

    #[test]
    fn metadados_leem_textos_e_png() {
        let mut d = pacote(b"CON ", TITLE_ID_RE5, 1);
        d[OFF_VERSAO_META + 3] = 2;
        let nome: Vec<u8> = "RESIDENT EVIL 5 SAVEDATA".encode_utf16().flat_map(u16::to_be_bytes).collect();
        let o = OFF_NOMES + 0x100; // primeiro idioma vazio, segundo preenchido
        d[o..o + nome.len()].copy_from_slice(&nome);
        d[OFF_TAM_MINI..OFF_TAM_MINI + 4].copy_from_slice(&16u32.to_be_bytes());
        d[OFF_MINI..OFF_MINI + 8].copy_from_slice(PNG);
        d[OFF_TAM_MINI_JOGO..OFF_TAM_MINI_JOGO + 4].copy_from_slice(&0x5000u32.to_be_bytes()); // acima do máximo
        let m = metadados(&d);
        assert_eq!(m.nome, "RESIDENT EVIL 5 SAVEDATA");
        assert_eq!(m.title_id, TITLE_ID_RE5);
        assert_eq!(m.miniatura.map(|b| b.len()), Some(16));
        assert!(m.miniatura_jogo.is_none());
    }

    /// Pacote versão 2 com nome só no idioma 8, como nos saves reais.
    fn pacote_com_nome() -> Vec<u8> {
        let mut d = pacote(b"CON ", TITLE_ID_RE5, 1);
        d[OFF_VERSAO_META + 3] = 2;
        let o = OFF_NOMES + 8 * TAM_NOME;
        let nome: Vec<u8> = "RESIDENT EVIL 5 SAVEDATA".encode_utf16().flat_map(u16::to_be_bytes).collect();
        d[o..o + nome.len()].copy_from_slice(&nome);
        d
    }

    #[test]
    fn nome_vai_so_para_os_idiomas_preenchidos() {
        let antes = pacote_com_nome();
        let mut d = antes.clone();
        grava_nome(&mut d, "  MEU SAVE ç  ").unwrap();
        assert_eq!(metadados(&d).nome, "MEU SAVE ç");
        let o = OFF_NOMES + 8 * TAM_NOME;
        let mudou: Vec<usize> = (0..d.len()).filter(|&k| d[k] != antes[k]).collect();
        assert!(mudou.iter().all(|&k| (o..o + TAM_NOME).contains(&k)), "mudou fora do idioma 8");
        // o resto do nome antigo, mais longo, foi apagado
        assert!(d[o + 2 * "MEU SAVE ç".encode_utf16().count()..o + TAM_NOME].iter().all(|&b| b == 0));
    }

    #[test]
    fn nome_sem_idioma_preenchido_vai_para_os_9() {
        let mut d = pacote(b"CON ", TITLE_ID_RE5, 1);
        d[OFF_VERSAO_META + 3] = 2;
        grava_nome(&mut d, "NOVO").unwrap();
        for i in 0..9 {
            assert_eq!(utf16_fixo(&d, OFF_NOMES + i * TAM_NOME), "NOVO");
        }
        for i in 0..3 {
            assert_eq!(utf16_fixo(&d, OFF_NOMES_EXTRA + i * TAM_NOME), "");
        }
    }

    #[test]
    fn nome_invalido_e_recusado_sem_mudar_nada() {
        let antes = pacote_com_nome();
        for ruim in ["", "   ", "com\nquebra", &"X".repeat(129)] {
            let mut d = antes.clone();
            assert!(grava_nome(&mut d, ruim).is_err(), "aceitou {ruim:?}");
            assert_eq!(d, antes);
        }
        let mut d = antes.clone();
        assert!(grava_nome(&mut d, &"X".repeat(128)).is_ok());
    }

    #[test]
    fn bloco_fora_da_tabela_e_erro_e_nao_panico() {
        let mut d = pacote(b"CON ", TITLE_ID_RE5, 1);
        d[0x340..0x344].copy_from_slice(&0x971Au32.to_be_bytes());
        d[VD + 5] = 0xFF; // diretório num bloco que não existe
        let p = Stfs::novo(d).unwrap();
        assert!(p.le().is_err());
        assert!(!p.hashes_ok());
    }
}

#[cfg(test)]
mod testes_inventario {
    use super::*;

    /// Bytes pseudoaleatórios que cobrem o save inteiro.
    fn ruido() -> Vec<u8> {
        let mut x = 0x2545_F491_4F6C_DD1Du64;
        (0..0x5D50).map(|_| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; x as u8 }).collect()
    }

    #[test]
    fn checksum_do_xbox_termina_em_0x4bc0() {
        // No Xbox, o trecho [0x4BC0, 0x52B0) não entra no checksum (conferido
        // em saves gravados pelo console); no PC ele entra.
        let d = ruido();
        let xbox = Interno::xbox(d.clone());
        let pc = Interno::pc(d.clone());
        let mut d2 = d.clone();
        d2[0x4BC0] ^= 0xFF;
        d2[0x52AC] ^= 0x01;
        assert_eq!(Interno::xbox(d2.clone()).checksum(), xbox.checksum());
        assert_ne!(Interno::pc(d2).checksum(), pc.checksum());
        // e tudo antes de 0x4BC0 (fora a palavra pulada em 0x3BAC) entra
        let mut d3 = d.clone();
        d3[0x4BBC] ^= 0x01;
        assert_ne!(Interno::xbox(d3).checksum(), xbox.checksum());
        let mut d4 = d;
        d4[0x3BAC] ^= 0x01;
        assert_eq!(Interno::xbox(d4).checksum(), xbox.checksum());
    }

    #[test]
    fn xbox_le_os_mesmos_enderecos_de_antes() {
        let d = ruido();
        let i = Interno::xbox(d.clone());
        let inv = i.inventario();
        assert_eq!(inv.len(), 84);
        for s in &inv {
            let o = 0x3158 + s.slot * 12;
            assert_eq!(s.id, u16::from_be_bytes([d[o], d[o + 1]]) as u32);
            assert_eq!(s.amount, u16::from_be_bytes([d[o + 2], d[o + 3]]) as u32);
        }
        for (base, slots) in [(0x3690, i.personagem(i.off_chris)), (0x3AB0, i.personagem(i.off_sheva))] {
            for s in slots {
                assert_eq!(s.id, be32(&d, base + s.slot * 0x2C));
                assert_eq!(s.amount, be32(&d, base + s.slot * 0x2C + 4));
            }
        }
    }

    #[test]
    fn pc_grava_so_os_slots_pedidos() {
        let d = ruido();
        let mut i = Interno::pc(d.clone());
        assert_eq!(i.inventario().len(), 74);
        let m = Mudancas {
            inventario: vec![Slot { slot: 73, id: 0x102, amount: 99 }],
            chris: vec![Slot { slot: 0, id: 0x113, amount: 150 }],
            sheva: vec![Slot { slot: 8, id: 0x104, amount: 7 }],
            ..Default::default()
        };
        i.aplica_inventario(&m).unwrap();
        let mudou: Vec<usize> = (0..d.len()).filter(|&k| d[k] != i.d[k]).collect();
        let permitido = |k: usize| {
            (0x384C + 73 * 12..0x384C + 73 * 12 + 4).contains(&k)
                || (0x3D80..0x3D88).contains(&k)
                || (0x41A0 + 8 * 0x2C..0x41A0 + 8 * 0x2C + 8).contains(&k)
        };
        assert!(mudou.iter().all(|&k| permitido(k)), "mudou fora dos slots: {mudou:x?}");
        assert_eq!(&i.d[0x384C + 73 * 12..][..4], &[0x02, 0x01, 99, 0]);
        assert_eq!(&i.d[0x3D80..0x3D88], &[0x13, 0x01, 0, 0, 150, 0, 0, 0]);
        let fora = Mudancas { inventario: vec![Slot { slot: 74, id: 1, amount: 1 }], ..Default::default() };
        assert!(i.aplica_inventario(&fora).is_err());
    }
}
