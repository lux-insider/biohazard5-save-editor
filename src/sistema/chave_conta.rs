//! Chave de console de varejo usada para decifrar o arquivo "Account" do
//! pacote de perfil (HMAC-SHA1 + RC4). Só serve para ler a gamertag.
//! Não editar à mão.

/// Chave HMAC do "Account" (16 bytes).
pub const CONTA: [u8; 16] = [
    0xE1, 0xBC, 0x15, 0x9C, 0x73, 0xB1, 0xEA, 0xE9, 0xAB, 0x31, 0x70, 0xF3, 0xAD, 0x47, 0xEB, 0xF3,
];
