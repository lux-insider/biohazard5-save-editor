//! Leitor de zip só com o que o programa precisa: listar os arquivos e ler o
//! começo de cada um, sem compressão ou com deflate, com zip64.
//!
//! Toda posição e todo tamanho lido do zip é conferido contra o tamanho do
//! arquivo antes de usar, e nada é lido além do limite pedido: um zip
//! estragado ou feito de propósito para enganar vira um erro, nunca um
//! travamento ou uma leitura de gigabytes.
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use miniz_oxide::inflate::stream::{inflate, InflateState};
use miniz_oxide::{DataFormat, MZError, MZFlush, MZStatus};

const FIM: [u8; 4] = *b"PK\x05\x06";
const FIM64: [u8; 4] = *b"PK\x06\x06";
const LOCALIZADOR64: [u8; 4] = *b"PK\x06\x07";
const CENTRAL: [u8; 4] = *b"PK\x01\x02";
const LOCAL: [u8; 4] = *b"PK\x03\x04";
const TAM_FIM: u64 = 22;
const TAM_FIM64: u64 = 56;
const TAM_LOCALIZADOR64: u64 = 20;
const TAM_CENTRAL: usize = 46;
const TAM_LOCAL: u64 = 30;
/// O comentário do fim do zip tem no máximo 64 KB.
const COMENTARIO_MAXIMO: u64 = 0xFFFF;
/// Diretório central maior que isso não é de um backup de NAND.
const DIRETORIO_MAXIMO: u64 = 64 << 20;
const PEDACO: usize = 64 * 1024;

/// Um arquivo dentro do zip.
#[derive(Clone, Debug)]
pub struct Membro {
    /// O nome completo dentro do zip, com `/` separando as pastas.
    pub nome: String,
    /// Tamanho descompactado.
    pub tamanho: u64,
    compactado: u64,
    metodo: u16,
    flags: u16,
    crc: u32,
    cabecalho: u64,
    nome_bruto: Vec<u8>,
}

impl Membro {
    pub fn e_pasta(&self) -> bool {
        self.nome.ends_with('/')
    }
}

#[derive(Debug)]
pub enum ErroZip {
    /// Não tem o registro do fim do zip.
    NaoEZip,
    Estragado(&'static str),
    Senha,
    Metodo(u16),
    Io(io::Error),
}

impl From<io::Error> for ErroZip {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            ErroZip::Estragado("o zip acaba antes do que diz o diretório")
        } else {
            ErroZip::Io(e)
        }
    }
}

fn ler_em(f: &mut File, posicao: u64, tamanho: usize) -> Result<Vec<u8>, ErroZip> {
    f.seek(SeekFrom::Start(posicao))?;
    let mut v = vec![0u8; tamanho];
    f.read_exact(&mut v)?;
    Ok(v)
}

fn u16_em(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn u32_em(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn u64_em(b: &[u8], o: usize) -> u64 {
    let mut x = [0u8; 8];
    x.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(x)
}

/// O registro do fim do zip (o diretório central diz onde está cada arquivo).
struct Fim {
    tamanho_dir: u64,
    inicio_dir: u64,
    /// Onde está o registro do fim no arquivo.
    posicao: u64,
    zip64: bool,
}

/// Procura o registro do fim como o zipfile do Python: primeiro nos últimos
/// 22 bytes (zip sem comentário), depois nos últimos 64 KB.
fn achar_fim(f: &mut File, tamanho: u64) -> Result<Option<Fim>, ErroZip> {
    if tamanho < TAM_FIM {
        return Ok(None);
    }
    let comeco = tamanho.saturating_sub(TAM_FIM + COMENTARIO_MAXIMO);
    let cauda = ler_em(f, comeco, (tamanho - comeco) as usize)?;
    let ultimo = cauda.len() - TAM_FIM as usize;
    let posicao = if cauda[ultimo..ultimo + 4] == FIM && cauda[cauda.len() - 2..] == [0, 0] {
        ultimo
    } else {
        match cauda.windows(4).rposition(|w| w == FIM) {
            Some(i) if i + TAM_FIM as usize <= cauda.len() => i,
            _ => return Ok(None),
        }
    };
    let r = &cauda[posicao..posicao + TAM_FIM as usize];
    let mut fim = Fim {
        tamanho_dir: u64::from(u32_em(r, 12)),
        inicio_dir: u64::from(u32_em(r, 16)),
        posicao: comeco + posicao as u64,
        zip64: false,
    };
    // zip64: o localizador fica logo antes do registro do fim, e o registro
    // de 64 bits logo antes do localizador.
    if fim.posicao >= TAM_LOCALIZADOR64 + TAM_FIM64 {
        let loc = ler_em(f, fim.posicao - TAM_LOCALIZADOR64, TAM_LOCALIZADOR64 as usize)?;
        if loc[..4] == LOCALIZADOR64 {
            if u32_em(&loc, 4) != 0 || u32_em(&loc, 16) > 1 {
                return Err(ErroZip::Estragado("zip dividido em várias partes"));
            }
            let r64 = ler_em(f, fim.posicao - TAM_LOCALIZADOR64 - TAM_FIM64, TAM_FIM64 as usize)?;
            if r64[..4] == FIM64 {
                fim.tamanho_dir = u64_em(&r64, 40);
                fim.inicio_dir = u64_em(&r64, 48);
                fim.zip64 = true;
            }
        }
    }
    Ok(Some(fim))
}

/// Tem o registro do fim de um zip (o zipfile.is_zipfile do Python).
pub fn tem_fim(caminho: &Path) -> io::Result<bool> {
    let mut f = File::open(caminho)?;
    let tamanho = f.metadata()?.len();
    match achar_fim(&mut f, tamanho) {
        Ok(fim) => Ok(fim.is_some()),
        Err(ErroZip::Io(e)) => Err(e),
        Err(_) => Ok(true),
    }
}

/// Os arquivos do zip, na ordem do diretório central.
pub fn listar(caminho: &Path) -> Result<Vec<Membro>, ErroZip> {
    let mut f = File::open(caminho)?;
    let tamanho = f.metadata()?.len();
    let fim = achar_fim(&mut f, tamanho)?.ok_or(ErroZip::NaoEZip)?;
    let antes_do_fim = if fim.zip64 { TAM_FIM64 + TAM_LOCALIZADOR64 } else { 0 };
    // Como o Python: o diretório fica logo antes do registro do fim, e a
    // diferença para a posição anotada é o que veio antes do zip (um .exe
    // que se extrai sozinho, por exemplo).
    let inicio_dir = fim
        .posicao
        .checked_sub(antes_do_fim)
        .and_then(|p| p.checked_sub(fim.tamanho_dir))
        .ok_or(ErroZip::Estragado("posição do diretório central"))?;
    let deslocamento = i128::from(inicio_dir) - i128::from(fim.inicio_dir);
    if fim.tamanho_dir > DIRETORIO_MAXIMO {
        return Err(ErroZip::Estragado("diretório central grande demais"));
    }
    let dir = ler_em(&mut f, inicio_dir, fim.tamanho_dir as usize)?;
    let mut membros = Vec::new();
    let mut p = 0usize;
    while p < dir.len() {
        let c = dir.get(p..p + TAM_CENTRAL).ok_or(ErroZip::Estragado("diretório central cortado"))?;
        if c[..4] != CENTRAL {
            return Err(ErroZip::Estragado("diretório central"));
        }
        let flags = u16_em(c, 8);
        let n = usize::from(u16_em(c, 28));
        let e = usize::from(u16_em(c, 30));
        let k = usize::from(u16_em(c, 32));
        let resto = dir.get(p + TAM_CENTRAL..p + TAM_CENTRAL + n + e + k).ok_or(ErroZip::Estragado("diretório central cortado"))?;
        let nome_bruto = resto[..n].to_vec();
        let mut membro = Membro {
            nome: decodificar_nome(&nome_bruto, flags & 0x800 != 0),
            tamanho: u64::from(u32_em(c, 24)),
            compactado: u64::from(u32_em(c, 20)),
            metodo: u16_em(c, 10),
            flags,
            crc: u32_em(c, 16),
            cabecalho: u64::from(u32_em(c, 42)),
            nome_bruto,
        };
        ler_extra_zip64(&mut membro, &resto[n..n + e])?;
        membro.cabecalho = u64::try_from(i128::from(membro.cabecalho) + deslocamento)
            .map_err(|_| ErroZip::Estragado("posição de um arquivo"))?;
        membros.push(membro);
        p += TAM_CENTRAL + n + e + k;
    }
    Ok(membros)
}

/// Tamanhos e posição de 64 bits, no campo extra 0x0001.
fn ler_extra_zip64(m: &mut Membro, mut extra: &[u8]) -> Result<(), ErroZip> {
    while extra.len() >= 4 {
        let tipo = u16_em(extra, 0);
        let tam = usize::from(u16_em(extra, 2));
        let dados = extra.get(4..4 + tam).ok_or(ErroZip::Estragado("campo extra"))?;
        if tipo == 0x0001 {
            let mut d = dados;
            let mut proximo = || -> Result<u64, ErroZip> {
                let v = d.get(..8).ok_or(ErroZip::Estragado("campo extra do zip64"))?;
                let v = u64_em(v, 0);
                d = &d[8..];
                Ok(v)
            };
            if m.tamanho == 0xFFFF_FFFF {
                m.tamanho = proximo()?;
            }
            if m.compactado == 0xFFFF_FFFF {
                m.compactado = proximo()?;
            }
            if m.cabecalho == 0xFFFF_FFFF {
                m.cabecalho = proximo()?;
            }
        }
        extra = &extra[4 + tam..];
    }
    Ok(())
}

/// Nome em UTF-8 quando o zip avisa; senão, na página de código 437 (a do DOS).
fn decodificar_nome(bruto: &[u8], utf8: bool) -> String {
    let bruto = &bruto[..bruto.iter().position(|&b| b == 0).unwrap_or(bruto.len())];
    if utf8 {
        return String::from_utf8_lossy(bruto).into_owned();
    }
    bruto.iter().map(|&b| if b < 0x80 { char::from(b) } else { CP437[usize::from(b - 0x80)] }).collect()
}

const CP437: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', //
    'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', //
    'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', //
    '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', //
    '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', //
    '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀', //
    'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', //
    '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{A0}',
];

/// CRC-32 do zip (o mesmo do PNG e do gzip).
fn crc32(dados: &[u8]) -> u32 {
    const TABELA: [u32; 256] = {
        let mut t = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut c = i as u32;
            let mut k = 0;
            while k < 8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
                k += 1;
            }
            t[i] = c;
            i += 1;
        }
        t
    };
    !dados.iter().fold(!0u32, |c, &b| TABELA[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8))
}

/// Lê no máximo `limite` bytes do começo de um arquivo do zip. Quando o
/// arquivo cabe inteiro no limite, confere também o CRC.
pub fn ler(caminho: &Path, m: &Membro, limite: usize) -> Result<Vec<u8>, ErroZip> {
    let mut f = File::open(caminho)?;
    let tamanho_zip = f.metadata()?.len();
    if m.cabecalho.saturating_add(TAM_LOCAL) > tamanho_zip {
        return Err(ErroZip::Estragado("cabeçalho de um arquivo fora do zip"));
    }
    let cab = ler_em(&mut f, m.cabecalho, TAM_LOCAL as usize)?;
    if cab[..4] != LOCAL {
        return Err(ErroZip::Estragado("cabeçalho de um arquivo"));
    }
    let n = u64::from(u16_em(&cab, 26));
    let e = u64::from(u16_em(&cab, 28));
    if m.flags & 0x20 != 0 {
        return Err(ErroZip::Metodo(m.metodo));
    }
    if m.flags & 0x40 != 0 {
        return Err(ErroZip::Senha);
    }
    let nome_local = ler_em(&mut f, m.cabecalho + TAM_LOCAL, n as usize)?;
    if nome_local != m.nome_bruto {
        return Err(ErroZip::Estragado("o nome no diretório e no cabeçalho são diferentes"));
    }
    if m.flags & 0x01 != 0 {
        return Err(ErroZip::Senha);
    }
    let inicio = m.cabecalho + TAM_LOCAL + n + e;
    let quer = usize::try_from(m.tamanho).map_or(limite, |t| t.min(limite));
    if inicio.saturating_add(m.compactado) > tamanho_zip {
        return Err(ErroZip::Estragado("o zip acaba antes do que diz o diretório"));
    }
    f.seek(SeekFrom::Start(inicio))?;
    let dados = match m.metodo {
        0 => {
            if m.compactado < quer as u64 {
                return Err(ErroZip::Estragado("arquivo cortado"));
            }
            let mut v = vec![0u8; quer];
            f.read_exact(&mut v)?;
            v
        }
        8 => descomprimir(&mut f, m.compactado, quer)?,
        outro => return Err(ErroZip::Metodo(outro)),
    };
    if dados.len() as u64 == m.tamanho && crc32(&dados) != m.crc {
        return Err(ErroZip::Estragado("CRC errado"));
    }
    Ok(dados)
}

/// Deflate até ter `quer` bytes, lendo no máximo `compactado` bytes do zip.
fn descomprimir(f: &mut File, compactado: u64, quer: usize) -> Result<Vec<u8>, ErroZip> {
    let mut estado = InflateState::new_boxed(DataFormat::Raw);
    let mut saida = vec![0u8; quer];
    let mut escrito = 0;
    let mut entrada = vec![0u8; PEDACO];
    let (mut ini, mut fim) = (0, 0);
    let mut falta = compactado;
    while escrito < quer {
        if ini == fim {
            if falta == 0 {
                return Err(ErroZip::Estragado("os dados comprimidos acabam antes do fim"));
            }
            let n = falta.min(PEDACO as u64) as usize;
            f.read_exact(&mut entrada[..n])?;
            (ini, fim) = (0, n);
            falta -= n as u64;
        }
        let r = inflate(&mut estado, &entrada[ini..fim], &mut saida[escrito..], MZFlush::None);
        ini += r.bytes_consumed;
        escrito += r.bytes_written;
        match r.status {
            Ok(MZStatus::StreamEnd) => break,
            Ok(_) | Err(MZError::Buf) => {
                if r.bytes_consumed == 0 && r.bytes_written == 0 && ini < fim {
                    return Err(ErroZip::Estragado("dados comprimidos"));
                }
            }
            Err(_) => return Err(ErroZip::Estragado("dados comprimidos")),
        }
    }
    if escrito < quer {
        return Err(ErroZip::Estragado("os dados comprimidos acabam antes do fim"));
    }
    Ok(saida)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn crc32_conhecido() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn nomes_cp437_e_utf8() {
        assert_eq!(decodificar_nome(b"c\x87pia.bin", false), "cçpia.bin");
        assert_eq!(decodificar_nome("cópia.bin".as_bytes(), true), "cópia.bin");
        assert_eq!(decodificar_nome(b"nand.bin\0lixo", false), "nand.bin");
    }
}
