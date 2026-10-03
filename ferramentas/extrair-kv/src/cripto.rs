//! HMAC-SHA1, RC4 e SHA-256: o que o console usa no keyvault.
use sha1::{Digest, Sha1};
use sha2::Sha256;

/// HMAC-SHA1 (RFC 2104) das partes, como se estivessem juntas, sem copiá-las.
pub fn hmac_sha1(chave: &[u8], partes: &[&[u8]]) -> [u8; 20] {
    let mut bloco = [0u8; 64];
    if chave.len() > bloco.len() {
        bloco[..20].copy_from_slice(&Sha1::digest(chave));
    } else {
        bloco[..chave.len()].copy_from_slice(chave);
    }
    let mut interno = Sha1::new();
    interno.update(bloco.map(|b| b ^ 0x36));
    for parte in partes {
        interno.update(parte);
    }
    let mut externo = Sha1::new();
    externo.update(bloco.map(|b| b ^ 0x5C));
    externo.update(interno.finalize());
    externo.finalize().into()
}

/// RC4 com chave de 16 bytes: cifra e decifra no lugar (é o mesmo XOR).
pub fn rc4(chave: &[u8; 16], dados: &mut [u8]) {
    let mut s: [u8; 256] = std::array::from_fn(|i| i as u8);
    let mut j = 0u8;
    for i in 0..256 {
        j = j.wrapping_add(s[i]).wrapping_add(chave[i % chave.len()]);
        s.swap(i, usize::from(j));
    }
    let (mut i, mut j) = (0u8, 0u8);
    for b in dados {
        i = i.wrapping_add(1);
        j = j.wrapping_add(s[usize::from(i)]);
        s.swap(usize::from(i), usize::from(j));
        *b ^= s[usize::from(s[usize::from(i)].wrapping_add(s[usize::from(j)]))];
    }
}

/// Compara sem parar no primeiro byte diferente, como o hmac.compare_digest.
pub fn iguais(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |dif, (x, y)| dif | (x ^ y)) == 0
}

/// SHA-256 em hexadecimal minúsculo, para conferir o kv.bin gravado.
pub fn sha256_hex(dados: &[u8]) -> String {
    Sha256::digest(dados).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn hmac_sha1_vetores_da_rfc_2202() {
        assert_eq!(hex(&hmac_sha1(&[0x0b; 20], &[b"Hi There"])), "b617318655057264e28bc0b6fb378c8ef146be00");
        assert_eq!(
            hex(&hmac_sha1(b"Jefe", &[b"what do ya want ", b"for nothing?"])),
            "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"
        );
        assert_eq!(hex(&hmac_sha1(&[0xaa; 20], &[&[0xdd; 50]])), "125d7342b9ac11cd91a39af48aa17b4f63f175d3");
        // chave maior que o bloco: entra o SHA-1 dela
        assert_eq!(
            hex(&hmac_sha1(&[0xaa; 80], &[b"Test Using Larger Than Block-Size Key - Hash Key First"])),
            "aa4ae5e15272d00e95705637ce8a3b55ed402112"
        );
    }

    #[test]
    fn rc4_vetores_da_rfc_6229() {
        let chave: [u8; 16] = std::array::from_fn(|i| i as u8 + 1);
        let mut fluxo = [0u8; 16];
        rc4(&chave, &mut fluxo);
        assert_eq!(hex(&fluxo), "9ac7cc9a609d1ef7b2932899cde41b97");
        let mut dados = *b"o keyvault volta";
        rc4(&chave, &mut dados);
        rc4(&chave, &mut dados);
        assert_eq!(&dados, b"o keyvault volta");
    }

    #[test]
    fn iguais_confere_tudo() {
        assert!(iguais(b"abc", b"abc"));
        assert!(!iguais(b"abc", b"abd"));
        assert!(!iguais(b"abc", b"ab"));
    }

    #[test]
    fn sha256_conhecido() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
