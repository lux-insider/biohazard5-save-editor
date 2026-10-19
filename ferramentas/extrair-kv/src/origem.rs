//! De onde vêm os arquivos: soltos, de uma pasta ou de dentro de um zip.
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::erro::{motivo, Erro};
use crate::nand::{CpuKey, MAGIC, NAND_MINIMA, TEXTO_MAXIMO};
use crate::texto::{absoluto, limpo, mostrar};
use crate::zip::{self, ErroZip, Membro};

/// Um arquivo solto, de uma pasta ou de dentro de um zip.
#[derive(Clone, Debug)]
pub struct Arquivo {
    /// Só o nome, sem a pasta.
    pub nome: String,
    pub tamanho: u64,
    fonte: Fonte,
}

#[derive(Clone, Debug)]
enum Fonte {
    Disco(PathBuf),
    Zip(Arc<Zip>, usize),
}

#[derive(Debug)]
struct Zip {
    caminho: PathBuf,
    membros: Vec<Membro>,
}

impl Arquivo {
    /// Lê no máximo `limite` bytes do começo do arquivo.
    pub fn ler(&self, limite: usize) -> Result<Vec<u8>, Erro> {
        match &self.fonte {
            Fonte::Disco(caminho) => ler_do_disco(caminho, limite),
            Fonte::Zip(z, i) => zip::ler(&z.caminho, &z.membros[*i], limite).map_err(|e| erro_de_membro(e, &self.nome, &z.caminho)),
        }
    }

    /// O caminho no disco, se o arquivo não está dentro de um zip.
    pub fn caminho(&self) -> Option<&Path> {
        match &self.fonte {
            Fonte::Disco(c) => Some(c),
            Fonte::Zip(..) => None,
        }
    }

    /// É o mesmo arquivo (o mesmo caminho, ou o mesmo membro do mesmo zip).
    pub fn mesmo(&self, outro: &Arquivo) -> bool {
        match (&self.fonte, &outro.fonte) {
            (Fonte::Disco(a), Fonte::Disco(b)) => a == b,
            (Fonte::Zip(a, i), Fonte::Zip(b, j)) => Arc::ptr_eq(a, b) && i == j,
            _ => false,
        }
    }
}

fn erro_de_membro(e: ErroZip, nome: &str, zip: &Path) -> Erro {
    let nome = limpo(nome);
    match e {
        ErroZip::Senha => Erro(format!(
            "o zip tem senha ({nome} está protegido). Extraia os arquivos e rode de novo com eles soltos."
        )),
        ErroZip::Metodo(m) => Erro(format!(
            "o zip usa um tipo de compressão que este programa não lê (método {m}, em {nome}). \
             Extraia os arquivos e rode de novo com eles soltos."
        )),
        ErroZip::Estragado(onde) => Erro(format!(
            "o zip está estragado ({onde}, em {nome}). Copie ou baixe o zip de novo, ou extraia os arquivos."
        )),
        ErroZip::NaoEZip => Erro(format!("o arquivo não é um zip válido: {}", mostrar(zip))),
        ErroZip::Io(e) => Erro(format!("não consegui ler {}: {}", mostrar(zip), motivo(&e))),
    }
}

fn ler_do_disco(caminho: &Path, limite: usize) -> Result<Vec<u8>, Erro> {
    let erro = |e: io::Error| Erro(format!("não consegui ler {}: {}", mostrar(caminho), motivo(&e)));
    let f = File::open(caminho).map_err(erro)?;
    let mut dados = Vec::new();
    f.take(limite as u64).read_to_end(&mut dados).map_err(erro)?;
    Ok(dados)
}

fn nome_de(caminho: &Path) -> String {
    caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Um arquivo do disco, com o tamanho que ele tem agora.
pub fn do_disco(caminho: &Path, tamanho: u64) -> Arquivo {
    Arquivo { nome: nome_de(caminho), tamanho, fonte: Fonte::Disco(caminho.to_path_buf()) }
}

/// Os arquivos comuns de uma pasta (sem seguir atalhos), em ordem de nome.
fn arquivos_da_pasta(pasta: &Path, fora: Option<&Path>) -> Vec<Arquivo> {
    let Ok(entradas) = fs::read_dir(pasta) else {
        return Vec::new();
    };
    let mut achados: Vec<Arquivo> = entradas
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| fora.is_none_or(|f| e.path() != f))
        .filter_map(|e| Some(do_disco(&e.path(), e.metadata().ok()?.len())))
        .collect();
    achados.sort_by(|a, b| a.nome.cmp(&b.nome));
    achados
}

/// A pasta e as subpastas dela, sem descer mais.
pub fn da_pasta(pasta: &Path) -> Vec<Arquivo> {
    let mut achados = arquivos_da_pasta(pasta, None);
    let mut subpastas: Vec<PathBuf> = fs::read_dir(pasta)
        .map(|entradas| {
            entradas.filter_map(Result::ok).filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()).collect()
        })
        .unwrap_or_default();
    subpastas.sort();
    for sub in subpastas {
        achados.extend(arquivos_da_pasta(&sub, None));
    }
    achados
}

/// Os arquivos de dentro de um zip (sem as pastas).
pub fn do_zip(caminho: &Path) -> Result<Vec<Arquivo>, Erro> {
    let membros = zip::listar(caminho).map_err(|e| match e {
        ErroZip::Io(e) => Erro(format!("não consegui ler {}: {}", mostrar(caminho), motivo(&e))),
        _ => Erro(format!("o arquivo não é um zip válido: {}", mostrar(caminho))),
    })?;
    let z = Arc::new(Zip { caminho: caminho.to_path_buf(), membros });
    Ok(z.membros
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.e_pasta())
        .map(|(i, m)| Arquivo {
            nome: m.nome.rsplit(['/', '\\']).next().unwrap_or_default().to_string(),
            tamanho: m.tamanho,
            fonte: Fonte::Zip(Arc::clone(&z), i),
        })
        .collect())
}

/// A cópia da NAND: começa com 0xFF e tem pelo menos o keyvault.
///
/// Entre as que começam com FF 4F, prefere os nomes das cópias do Simple 360
/// NAND Flasher (flashdmp.bin, recovery.bin) e do J-Runner (nanddump), deixa
/// por último o updflash.bin (a imagem que seria gravada) e, no empate, a maior.
pub fn escolher_nand(arquivos: &[Arquivo]) -> Result<Arquivo, Erro> {
    let nota = |a: &Arquivo| {
        let nome = a.nome.to_lowercase();
        let preferido = ["flashdmp", "nanddump", "recovery", "nand", "orig"].iter().any(|p| nome.contains(p));
        (preferido, !nome.contains("updflash"), a.tamanho)
    };
    let mut grandes = Vec::new();
    for a in arquivos.iter().filter(|a| a.tamanho >= NAND_MINIMA) {
        let comeco = a.ler(2)?;
        grandes.push((a, comeco));
    }
    let mut candidatos: Vec<&Arquivo> = grandes.iter().filter(|(_, c)| c[..] == MAGIC).map(|(a, _)| *a).collect();
    if candidatos.is_empty() {
        candidatos = grandes.iter().filter(|(_, c)| c.first() == Some(&0xFF)).map(|(a, _)| *a).collect();
    }
    // o primeiro com a maior nota, como o max() do Python
    let mut melhor: Option<&Arquivo> = None;
    for a in candidatos {
        if melhor.is_none_or(|m| nota(a) > nota(m)) {
            melhor = Some(a);
        }
    }
    melhor.cloned().ok_or_else(|| Erro::new("não achei uma cópia da NAND (um arquivo que comece com os bytes FF 4F)."))
}

/// As CPU keys de um texto: toda sequência de exatamente 32 caracteres
/// hexadecimais, também com espaços, tabs, dois-pontos ou traços no meio.
/// Não pega 32 caracteres do meio de um hash maior.
pub fn chaves_do_texto(texto: &[u8]) -> Vec<CpuKey> {
    let mut chaves = Vec::new();
    juntar_chaves(texto, &mut chaves);
    for linha in linhas(texto) {
        let sem_separadores: Vec<u8> = linha.iter().copied().filter(|b| !matches!(b, b' ' | b'\t' | b':' | b'-')).collect();
        juntar_chaves(&sem_separadores, &mut chaves);
    }
    chaves
}

fn juntar_chaves(texto: &[u8], chaves: &mut Vec<CpuKey>) {
    for trecho in texto.split(|b| !b.is_ascii_hexdigit()) {
        if trecho.len() == 32 {
            let chave: CpuKey = std::array::from_fn(|i| (valor_hex(trecho[2 * i]) << 4) | valor_hex(trecho[2 * i + 1]));
            if !chaves.contains(&chave) {
                chaves.push(chave);
            }
        }
    }
}

fn valor_hex(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        _ => c - b'A' + 10,
    }
}

/// As linhas do texto, separadas por \n, \r ou \r\n (como o bytes.splitlines).
fn linhas(texto: &[u8]) -> impl Iterator<Item = &[u8]> {
    texto.split(|&b| b == b'\n' || b == b'\r')
}

/// Sem repetir, na ordem em que apareceram, e no máximo `maximo`.
pub fn sem_repetir(chaves: Vec<CpuKey>, maximo: usize) -> Vec<CpuKey> {
    let mut saida: Vec<CpuKey> = Vec::new();
    for c in chaves {
        if saida.len() == maximo {
            break;
        }
        if !saida.contains(&c) {
            saida.push(c);
        }
    }
    saida
}

/// As CPU keys dos arquivos que estão junto com a NAND: primeiro os que têm
/// "cpu" e "key" no nome, depois os textos pequenos (.txt, .log, .ini, .cfg,
/// .json), como o log do Simple 360 NAND Flasher.
pub fn chaves_dos_arquivos(arquivos: &[Arquivo], nand: &Arquivo) -> Result<Vec<CpuKey>, Erro> {
    let nomeado = |a: &Arquivo| {
        let nome = a.nome.to_lowercase();
        nome.contains("cpu") && nome.contains("key")
    };
    let texto = |a: &Arquivo| {
        let nome = a.nome.to_lowercase();
        !nomeado(a)
            && !a.mesmo(nand)
            && a.tamanho <= TEXTO_MAXIMO as u64
            && [".txt", ".log", ".ini", ".cfg", ".json"].iter().any(|e| nome.ends_with(e))
    };
    let mut chaves = Vec::new();
    for a in arquivos.iter().filter(|a| nomeado(a)).chain(arquivos.iter().filter(|a| texto(a))) {
        chaves.extend(chaves_do_texto(&a.ler(TEXTO_MAXIMO)?));
    }
    Ok(chaves)
}

/// É um zip: começa como um zip, ou tem o registro do fim de um zip (um .exe
/// que se extrai sozinho também). Uma cópia da NAND (FF 4F) nunca é zip.
fn parece_zip(caminho: &Path, comeco: &[u8]) -> Result<bool, Erro> {
    if comeco.starts_with(&MAGIC) {
        return Ok(false);
    }
    if comeco.starts_with(b"PK\x03\x04") || comeco.starts_with(b"PK\x05\x06") {
        return Ok(true);
    }
    zip::tem_fim(caminho).map_err(|e| Erro(format!("não consegui ler {}: {}", mostrar(caminho), motivo(&e))))
}

/// A cópia da NAND e os arquivos ao lado dela, de um zip, de uma pasta ou de
/// um arquivo solto.
pub fn abrir_origem(origem: &Path) -> Result<(Arquivo, Vec<Arquivo>), Erro> {
    let meta = match fs::metadata(origem) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(Erro(format!("não achei: {}", mostrar(origem)))),
        Err(e) => return Err(Erro(format!("não consegui abrir {}: {}", mostrar(origem), motivo(&e)))),
    };
    if meta.is_dir() {
        let arquivos = da_pasta(origem);
        return Ok((escolher_nand(&arquivos)?, arquivos));
    }
    if !meta.is_file() {
        return Err(Erro(format!("não é um arquivo comum: {}", mostrar(origem))));
    }
    if parece_zip(origem, &ler_do_disco(origem, 4)?)? {
        let arquivos = do_zip(origem)?;
        return Ok((escolher_nand(&arquivos)?, arquivos));
    }
    let nand = do_disco(origem, meta.len());
    // a CPU key costuma estar ao lado
    let completo = absoluto(origem);
    let arquivos = completo.parent().map(|pasta| arquivos_da_pasta(pasta, Some(&completo))).unwrap_or_default();
    Ok((nand, arquivos))
}

#[cfg(test)]
mod testes {
    use super::*;

    const CPU: &str = "0123456789ABCDEF0123456789abcdef";

    fn chave(hex: &str) -> CpuKey {
        chaves_do_texto(hex.as_bytes())[0]
    }

    #[test]
    fn acha_a_chave_sozinha_e_com_separadores() {
        assert_eq!(chaves_do_texto(CPU.as_bytes()).len(), 1);
        assert_eq!(chaves_do_texto(b"Your CPUKey is: 0123456789ABCDEF0123456789ABCDEF\n").len(), 1);
        let espacada = b"CPU Key: 01234567 89ABCDEF 01234567 89ABCDEF\r\n";
        assert_eq!(chaves_do_texto(espacada), vec![chave("0123456789ABCDEF0123456789ABCDEF")]);
        let tracos = b"0123-4567-89AB-CDEF-0123-4567-89AB-CDEF";
        assert_eq!(chaves_do_texto(tracos).len(), 1);
    }

    #[test]
    fn nao_pega_pedaco_de_hash_maior() {
        assert!(chaves_do_texto(format!("{CPU}abcdef12").as_bytes()).is_empty());
        assert!(chaves_do_texto(format!("x{CPU}0").as_bytes()).is_empty());
        assert!(chaves_do_texto(&CPU.as_bytes()[1..]).is_empty());
    }

    #[test]
    fn maiusculas_e_minusculas_sao_a_mesma_chave() {
        let texto = format!("{}\n{}\n", CPU.to_uppercase(), CPU.to_lowercase());
        assert_eq!(chaves_do_texto(texto.as_bytes()).len(), 1);
    }

    #[test]
    fn sem_repetir_corta_no_maximo() {
        let chaves: Vec<CpuKey> = (0..10u8).map(|i| [i; 16]).chain([[0; 16]]).collect();
        assert_eq!(sem_repetir(chaves.clone(), 256).len(), 10);
        assert_eq!(sem_repetir(chaves, 3), vec![[0; 16], [1; 16], [2; 16]]);
    }
}
