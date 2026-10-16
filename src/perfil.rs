//! Gamertag do dono do save, lida do pacote de perfil do Xbox 360.
//!
//! O perfil é um pacote CON (tipo 0x10000, Title ID FFFE07D1) que fica em
//! `Content\<XUID>\FFFE07D1\00010000\<XUID>`. A gamertag está no arquivo
//! interno "Account" (404 bytes), cifrado:
//!
//! * os 16 primeiros bytes são um HMAC-SHA1 (cortado em 16) do resto;
//! * o resto é cifrado com RC4, cuja chave é o HMAC-SHA1 (cortado em 16) dos
//!   16 primeiros bytes; os dois HMAC usam a chave de `sistema/chave_conta.rs`;
//! * decifrado, o resto começa com 8 bytes aleatórios; depois vêm 4 bytes de
//!   flags, 4 reservados e a gamertag (16 caracteres UTF-16 big-endian).
//!
//! O "Account" também guarda dados sensíveis da conta (senha da Passport,
//! passcode, chave online): aqui só a gamertag sai desta função; o resto é
//! decifrado em memória e descartado.

use crate::save::{Result, Stfs};
use crate::sistema::chave_conta::CONTA;
use sha1::{Digest, Sha1};

const TIPO_PERFIL: u32 = 0x10000;
const TITLE_PERFIL: u32 = 0xFFFE07D1;
const TAM_ACCOUNT: usize = 404;

/// O que se tira do perfil: o XUID do pacote (o mesmo que os saves guardam
/// como "perfil") e a gamertag.
#[derive(Clone, Debug, PartialEq)]
pub struct Perfil {
    pub xuid: String,
    pub gamertag: String,
}

fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes(d[o..o + 4].try_into().unwrap())
}

/// HMAC-SHA1 (RFC 2104), bloco de 64 bytes.
fn hmac_sha1(chave: &[u8], dados: &[u8]) -> [u8; 20] {
    let mut k = [0u8; 64];
    if chave.len() > 64 {
        k[..20].copy_from_slice(&Sha1::digest(chave));
    } else {
        k[..chave.len()].copy_from_slice(chave);
    }
    let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5C).collect();
    let dentro = Sha1::new().chain_update(&ipad).chain_update(dados).finalize();
    Sha1::new().chain_update(&opad).chain_update(dentro).finalize().into()
}

/// RC4 (cifrar e decifrar são a mesma operação).
fn rc4(dados: &[u8], chave: &[u8]) -> Vec<u8> {
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

/// Decifra o "Account" e devolve só a gamertag. Erro se o HMAC não conferir
/// (arquivo corrompido ou de outro tipo de console).
fn gamertag_do_account(account: &[u8]) -> Result<String> {
    if account.len() != TAM_ACCOUNT {
        return Err("o arquivo Account do perfil tem tamanho inesperado".into());
    }
    let (cab, corpo) = account.split_at(16);
    let chave_rc4 = hmac_sha1(&CONTA, cab);
    let claro = rc4(corpo, &chave_rc4[..16]);
    if hmac_sha1(&CONTA, &claro)[..16] != *cab {
        return Err("não consegui decifrar o Account do perfil (HMAC não confere)".into());
    }
    // 8 bytes aleatórios, 4 de flags, 4 reservados, depois a gamertag
    let gt = &claro[16..16 + 32];
    let u: Vec<u16> = gt.chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).take_while(|&c| c != 0).collect();
    let nome = String::from_utf16_lossy(&u).trim().to_string();
    if nome.is_empty() {
        return Err("o perfil não tem gamertag".into());
    }
    Ok(nome)
}

/// Lê o pacote de perfil e devolve o XUID e a gamertag.
pub fn le_perfil(pkg: Vec<u8>) -> Result<Perfil> {
    if pkg.len() < 0x1000 || &pkg[..4] != b"CON " || be32(&pkg, 0x344) != TIPO_PERFIL || be32(&pkg, 0x360) != TITLE_PERFIL {
        return Err("esse arquivo não é um perfil do Xbox 360 (pasta FFFE07D1\\00010000)".into());
    }
    let xuid = pkg[0x371..0x379].iter().map(|b| format!("{b:02X}")).collect();
    let account = Stfs::abre_con(pkg)?.arquivo("Account")?;
    Ok(Perfil { xuid, gamertag: gamertag_do_account(&account)? })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn hmac_sha1_bate_com_a_rfc_2202() {
        assert_eq!(hmac_sha1(&[0x0b; 20], b"Hi There").to_vec(), hex("b617318655057264e28bc0b6fb378c8ef146be00"));
        assert_eq!(hmac_sha1(b"Jefe", b"what do ya want for nothing?").to_vec(), hex("effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"));
    }

    #[test]
    fn rc4_bate_com_o_vetor_conhecido() {
        assert_eq!(rc4(b"Plaintext", b"Key"), hex("bbf316e8d940af0ad3"));
        assert_eq!(rc4(&rc4(b"ida e volta", b"chave"), b"chave"), b"ida e volta");
    }

    /// Monta um Account cifrado como o console faz, com a gamertag pedida.
    fn account(gamertag: &str) -> Vec<u8> {
        let mut claro = vec![0x5Au8; TAM_ACCOUNT - 16];
        claro[8..16].fill(0);
        claro[16..48].fill(0);
        for (i, c) in gamertag.encode_utf16().enumerate() {
            claro[16 + i * 2..18 + i * 2].copy_from_slice(&c.to_be_bytes());
        }
        let cab = hmac_sha1(&CONTA, &claro);
        let chave_rc4 = hmac_sha1(&CONTA, &cab[..16]);
        let mut a = cab[..16].to_vec();
        a.extend(rc4(&claro, &chave_rc4[..16]));
        a
    }

    #[test]
    fn decifra_a_gamertag() {
        assert_eq!(gamertag_do_account(&account("ABadAvatar")).unwrap(), "ABadAvatar");
        assert_eq!(gamertag_do_account(&account("Gamer Tag 123")).unwrap(), "Gamer Tag 123");
    }

    #[test]
    fn account_alterado_ou_errado_e_recusado() {
        let mut a = account("ABadAvatar");
        a[100] ^= 1;
        assert!(gamertag_do_account(&a).is_err());
        assert!(gamertag_do_account(&a[..400]).is_err());
    }

    #[test]
    fn so_aceita_pacote_de_perfil() {
        let mut d = vec![0u8; 0x2000];
        d[..4].copy_from_slice(b"CON ");
        assert!(le_perfil(d.clone()).is_err()); // tipo e title zerados
        d[0x344..0x348].copy_from_slice(&1u32.to_be_bytes()); // save de jogo
        d[0x360..0x364].copy_from_slice(&0x434307D4u32.to_be_bytes());
        assert!(le_perfil(d).unwrap_err().contains("não é um perfil"));
    }
}
