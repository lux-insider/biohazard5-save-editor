//! Extrai o kv.bin (keyvault) de uma cópia da NAND do Xbox 360.
//!
//! Funciona com a cópia de qualquer placa, de qualquer tamanho: NAND de 16,
//! 64, 256 ou 512 MB, com ou sem os 16 bytes de ECC de cada página, e eMMC
//! de 4 GB (Corona 4 GB, Winchester), na cópia de 48 MB ou na imagem inteira.
//!
//! É a biblioteca do programa `extrair-kv` (linha de comando e menu), e pode
//! servir o editor:
//!
//! - [`abrir_origem`]: a cópia da NAND e os arquivos ao lado dela, de um zip,
//!   de uma pasta ou de um arquivo solto;
//! - [`chaves_dos_arquivos`] e [`chaves_do_texto`]: as CPU keys escritas nos
//!   arquivos (cpukey.txt, o log do Simple 360 NAND Flasher, anotações);
//! - [`abrir_kv`]: decifra o keyvault e confere o HMAC-SHA1 como o console
//!   confere (libxenon, kv_read);
//! - [`gravar`]: grava sem nunca sobrescrever, só para o dono, e confere.
//!
//! Tudo em Rust seguro: nenhum `unsafe` nesta biblioteca.
#![forbid(unsafe_code)]

mod cripto;
mod erro;
mod gravar;
mod nand;
mod origem;
mod texto;
mod zip;

pub use cripto::sha256_hex;
pub use erro::{motivo, Erro};
pub use gravar::gravar;
pub use nand::{
    abrir_kv, decifrar, descrever, ecc_confere, estrutura_confere, ler_logico, posicao_do_kv, serie_e_peca, Achado,
    CpuKey, CHAVES_MAXIMO, KV_PADRAO, KV_TAMANHO, LEITURA, MAGIC, NAND_MINIMA, PAGINA, PAGINA_ECC, TEXTO_MAXIMO,
};
pub use origem::{
    abrir_origem, chaves_do_texto, chaves_dos_arquivos, da_pasta, do_disco, do_zip, escolher_nand, sem_repetir,
    Arquivo,
};
pub use texto::{absoluto, limpo, mostrar};
