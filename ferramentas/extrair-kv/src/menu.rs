//! O menu: acha as cópias da NAND sozinho, pede só o que falta e grava o
//! kv.bin onde o editor procura.
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use extrair_kv::{
    abrir_kv, abrir_origem, absoluto, chaves_do_texto, chaves_dos_arquivos, descrever, do_disco, do_zip, escolher_nand,
    gravar, limpo, mostrar, serie_e_peca, sem_repetir, sha256_hex, Achado, Arquivo, CpuKey, Erro, CHAVES_MAXIMO,
    KV_PADRAO, KV_TAMANHO, LEITURA, MAGIC, NAND_MINIMA, PAGINA_ECC,
};

use crate::erro_danificado;
use crate::terminal::{cor, dizer, escrever, ler_segredo, ligar_cores, perguntar, perguntar_bruto, Falha};

/// Arquivos olhados no máximo em cada lugar.
const LIMITE_BUSCA: usize = 3000;
/// A pasta, as subpastas e as subpastas delas.
const PROFUNDIDADE: usize = 2;
const PULAR: [&str; 9] = [
    "node_modules",
    "$recycle.bin",
    "system volume information",
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "appdata",
    "__pycache__",
];
const NOMES_EDITOR: [&str; 2] = ["biohazard5-save-editor", "biohazard 5 save editor"];

const SEPARADOR: char = std::path::MAIN_SEPARATOR;

fn titulo(texto: &str) {
    let linha = "─".repeat(60);
    dizer!();
    dizer!("{}", cor(&linha, "36"));
    dizer!("{}", cor(&format!("  {texto}"), "1;36"));
    dizer!("{}", cor(&linha, "36"));
}

/// A pasta pessoal, como o os.path.expanduser("~") do Python.
fn casa() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from).or_else(std::env::home_dir)
}

/// Caminho para mostrar: a pasta pessoal vira ~.
fn curto(caminho: &Path) -> String {
    let caminho = absoluto(caminho).to_string_lossy().into_owned();
    if let Some(casa) = casa() {
        let casa = casa.to_string_lossy();
        let raiz = casa.is_empty() || casa.len() == 1 && casa.starts_with(SEPARADOR);
        if !raiz && (caminho == casa || caminho.starts_with(&format!("{casa}{SEPARADOR}"))) {
            return limpo(&format!("~{}", &caminho[casa.len()..]));
        }
    }
    limpo(&caminho)
}

/// Caminho colado ou arrastado para o terminal: tira aspas e entende o ~.
fn caminho_digitado(bruto: &[u8]) -> PathBuf {
    let mut texto = bruto.trim_ascii();
    if texto.len() >= 2 && texto[0] == texto[texto.len() - 1] && matches!(texto[0], b'\'' | b'"') {
        texto = &texto[1..texto.len() - 1];
    }
    let mut bytes = texto.to_vec();
    if cfg!(not(windows)) {
        // o terminal do Linux escapa os espaços ao arrastar: "Backup\ NAND.zip"
        let mut sem_escape = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\\' && bytes.get(i + 1) == Some(&b' ') {
                i += 1;
            }
            sem_escape.push(bytes[i]);
            i += 1;
        }
        bytes = sem_escape;
    }
    let caminho = caminho_de_bytes(bytes);
    let mut partes = caminho.components();
    if partes.next().is_some_and(|p| p.as_os_str() == "~") {
        if let Some(casa) = casa() {
            return casa.join(partes.as_path());
        }
    }
    caminho
}

#[cfg(unix)]
fn caminho_de_bytes(bytes: Vec<u8>) -> PathBuf {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn caminho_de_bytes(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(&bytes).into_owned())
}

/// O caminho real (sem atalhos), mostrado sem o \\?\ que o Windows põe.
fn real(caminho: &Path) -> Option<PathBuf> {
    let r = fs::canonicalize(caminho).ok()?;
    #[cfg(windows)]
    {
        let s = r.to_string_lossy();
        if let Some(resto) = s.strip_prefix(r"\\?\UNC\") {
            return Some(PathBuf::from(format!(r"\\{resto}")));
        }
        if let Some(resto) = s.strip_prefix(r"\\?\") {
            return Some(PathBuf::from(resto));
        }
    }
    Some(r)
}

/// Downloads, Área de Trabalho e Documentos como o Linux configurou (user-dirs.dirs).
fn pastas_xdg(casa: &Path) -> Vec<PathBuf> {
    let Ok(texto) = fs::read_to_string(casa.join(".config").join("user-dirs.dirs")) else {
        return Vec::new();
    };
    let mut pastas = Vec::new();
    for linha in texto.lines() {
        let Some((chave, valor)) = linha.trim().split_once('=') else {
            continue;
        };
        if matches!(chave, "XDG_DOWNLOAD_DIR" | "XDG_DESKTOP_DIR" | "XDG_DOCUMENTS_DIR") && !valor.is_empty() {
            pastas.push(PathBuf::from(valor.trim_matches('"').replace("$HOME", &casa.to_string_lossy())));
        }
    }
    pastas
}

/// Onde costumam estar as cópias: Downloads, Área de Trabalho, Documentos e pendrives.
fn lugares_de_busca() -> Vec<PathBuf> {
    let casa = casa().unwrap_or_default();
    let nomes = [
        "Downloads",
        "Transferências",
        "Desktop",
        "Área de Trabalho",
        "Documents",
        "Documentos",
        "OneDrive/Desktop",
        "OneDrive/Área de Trabalho",
        "OneDrive/Documentos",
        "OneDrive/Documents",
    ];
    let mut lugares: Vec<PathBuf> =
        nomes.iter().map(|n| n.split('/').fold(casa.clone(), |c, parte| c.join(parte))).collect();
    lugares.extend(pastas_xdg(&casa));
    // a pasta atual e a pasta do programa
    lugares.extend(std::env::current_dir());
    lugares.extend(std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)));
    if cfg!(windows) {
        // pendrives e HDs externos
        for letra in 'D'..='Z' {
            let raiz = PathBuf::from(format!("{letra}:\\"));
            if raiz.is_dir() {
                lugares.push(raiz);
            }
        }
    } else {
        let usuario = std::env::var("USER")
            .ok()
            .filter(|u| !u.is_empty())
            .or_else(|| casa.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        for base in [format!("/media/{usuario}"), format!("/run/media/{usuario}"), "/media".into(), "/mnt".into()] {
            let Ok(entradas) = fs::read_dir(&base) else {
                continue;
            };
            let mut pastas: Vec<PathBuf> = entradas
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .map(|e| e.path())
                .collect();
            pastas.sort();
            lugares.extend(pastas);
        }
    }
    let mut vistos = HashSet::new();
    let mut saida = Vec::new();
    for caminho in lugares {
        if let Some(r) = real(&caminho) {
            if r.is_dir() && vistos.insert(r.clone()) {
                saida.push(r);
            }
        }
    }
    saida
}

/// O executável do editor: biohazard5-save-editor no Linux, BIOHAZARD 5 SAVE EDITOR*.exe no Windows.
fn eh_editor(nome: &str) -> bool {
    let nome = nome.to_lowercase();
    // como o os.path.splitext: pontos no começo do nome não separam extensão
    let comeco = nome.len() - nome.trim_start_matches('.').len();
    let (base, ext) = match nome[comeco..].rfind('.') {
        Some(i) => nome.split_at(comeco + i),
        None => (nome.as_str(), ""),
    };
    matches!(ext, "" | ".exe") && NOMES_EDITOR.iter().any(|n| base.starts_with(n))
}

/// Os arquivos da pasta, até PROFUNDIDADE níveis abaixo, sem seguir atalhos,
/// com o tamanho e os atributos que a listagem da pasta já traz.
fn andar(raiz: &Path, limite: usize) -> Vec<(PathBuf, fs::Metadata)> {
    let mut achados = Vec::new();
    let mut pilha = vec![(raiz.to_path_buf(), 0)];
    while let Some((pasta, nivel)) = pilha.pop() {
        let Ok(entradas) = fs::read_dir(&pasta) else {
            continue;
        };
        let mut entradas: Vec<_> = entradas.filter_map(Result::ok).collect();
        entradas.sort_by_cached_key(|e| e.file_name().to_string_lossy().to_lowercase());
        for e in entradas {
            let Ok(tipo) = e.file_type() else {
                continue;
            };
            if tipo.is_symlink() {
                continue;
            }
            if tipo.is_dir() {
                let nome = e.file_name().to_string_lossy().into_owned();
                if nivel < PROFUNDIDADE && !nome.starts_with('.') && !PULAR.contains(&nome.to_lowercase().as_str()) {
                    pilha.push((e.path(), nivel + 1));
                }
            } else if tipo.is_file() {
                let Ok(meta) = e.metadata() else {
                    continue;
                };
                achados.push((e.path(), meta));
                if achados.len() >= limite {
                    return achados;
                }
            }
        }
    }
    achados
}

/// Arquivo que só está na nuvem (OneDrive e afins): ler o começo dele faria o
/// Windows baixar o arquivo inteiro. A busca pula; o caminho ainda pode ser
/// digitado na opção [c].
#[cfg(windows)]
fn so_na_nuvem(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const OFFLINE: u32 = 0x1000;
    const RECALL_ON_OPEN: u32 = 0x4_0000;
    const RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
    meta.file_attributes() & (OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS) != 0
}

#[cfg(not(windows))]
fn so_na_nuvem(_: &fs::Metadata) -> bool {
    false
}

/// Começa com FF 4F e o cabeçalho diz que o keyvault tem 16 KB: é uma NAND do Xbox 360.
fn parece_nand(cabeca: &[u8]) -> bool {
    let be32 = |o: usize| u32::from_be_bytes([cabeca[o], cabeca[o + 1], cabeca[o + 2], cabeca[o + 3]]) as usize;
    cabeca.len() >= 0x70 && cabeca[..2] == MAGIC && (be32(0x60) == KV_TAMANHO || be32(0x6C) == KV_PADRAO)
}

/// Os arquivos comuns da mesma pasta, menos o próprio.
fn vizinhos(pasta: &Path, menos: &Path) -> Vec<Arquivo> {
    let Ok(entradas) = fs::read_dir(pasta) else {
        return Vec::new();
    };
    let mut arquivos: Vec<Arquivo> = entradas
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()) && e.path() != menos)
        .filter_map(|e| Some(do_disco(&e.path(), e.metadata().ok()?.len())))
        .collect();
    arquivos.sort_by(|a, b| a.nome.cmp(&b.nome));
    arquivos
}

/// Uma cópia da NAND achada no computador, com o que dá para saber dela.
struct Copia {
    origem: PathBuf,
    lugar: PathBuf,
    nand: Arquivo,
    arquivos: Vec<Arquivo>,
    com_ecc: bool,
    achado: Option<Achado>,
    danificado: Option<Achado>,
    tem_chave: bool,
}

impl Copia {
    fn new(origem: PathBuf, lugar: PathBuf, nand: Arquivo, arquivos: Vec<Arquivo>) -> Self {
        Copia { origem, lugar, nand, arquivos, com_ecc: false, achado: None, danificado: None, tem_chave: false }
    }

    /// Lê o começo da cópia e tenta abrir o KV com as CPU keys que estão junto
    /// (ou com as digitadas).
    fn identificar(&mut self, digitadas: Option<Vec<CpuKey>>) -> Result<(), Erro> {
        let chaves = match digitadas {
            Some(c) => c,
            None => {
                let c = sem_repetir(chaves_dos_arquivos(&self.arquivos, &self.nand)?, CHAVES_MAXIMO);
                self.tem_chave = !c.is_empty();
                c
            }
        };
        let prefixo = self.nand.ler(LEITURA)?;
        self.com_ecc = self.nand.tamanho.is_multiple_of(PAGINA_ECC as u64);
        if prefixo.len() as u64 >= NAND_MINIMA && !chaves.is_empty() {
            (self.achado, self.danificado) = abrir_kv(&prefixo, self.nand.tamanho, &chaves);
            if let Some(info) = self.achado.as_ref().or(self.danificado.as_ref()) {
                self.com_ecc = info.com_ecc;
            }
        }
        Ok(())
    }

    fn linhas(&self) -> (String, String) {
        let tipo = descrever(self.nand.tamanho, self.com_ecc);
        let situacao = if let Some(a) = &self.achado {
            let (serie, peca) = serie_e_peca(&a.kv);
            let peca = if peca.is_empty() { String::new() } else { format!(" · peça {peca}") };
            cor(&format!("série {serie}{peca} · CPU key confere"), "32")
        } else if self.danificado.is_some() {
            cor("a CPU key é deste console, mas o KV da cópia está danificado", "31")
        } else if self.tem_chave {
            cor("a CPU key que está junto não abre esta cópia", "33")
        } else {
            cor("sem a CPU key junto: ela vai ser pedida", "2")
        };
        (tipo, situacao)
    }
}

/// Procura cópias da NAND e o editor nos lugares de costume. Reconhece pelo conteúdo, não pelo nome.
fn procurar() -> (Vec<Copia>, Vec<PathBuf>) {
    let mut copias = Vec::new();
    let mut editores: Vec<PathBuf> = Vec::new();
    let mut vistos = HashSet::new();
    for raiz in lugares_de_busca() {
        for (caminho, meta) in andar(&raiz, LIMITE_BUSCA) {
            let nome = caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let onde = caminho.parent().map(Path::to_path_buf).unwrap_or_default();
            if eh_editor(&nome) {
                if !editores.contains(&onde) {
                    editores.push(onde);
                }
                continue;
            }
            if meta.len() < NAND_MINIMA || so_na_nuvem(&meta) {
                continue;
            }
            let Some(r) = real(&caminho) else {
                continue;
            };
            if vistos.contains(&r) {
                continue;
            }
            let (nand, arquivos) = if nome.to_lowercase().ends_with(".zip") {
                match nand_do_zip(&caminho) {
                    Some(achado) => achado,
                    None => continue,
                }
            } else {
                let disco = do_disco(&caminho, meta.len());
                if !disco.ler(0x70).is_ok_and(|cabeca| parece_nand(&cabeca)) {
                    continue;
                }
                (disco, vizinhos(&onde, &caminho))
            };
            vistos.insert(r);
            copias.push(Copia::new(caminho, onde, nand, arquivos));
        }
    }
    for c in &mut copias {
        let _ = c.identificar(None);
    }
    (copias, editores)
}

/// A NAND dentro de um zip: olha só os 10 maiores arquivos dele.
fn nand_do_zip(caminho: &Path) -> Option<(Arquivo, Vec<Arquivo>)> {
    let arquivos = do_zip(caminho).ok()?;
    let mut grandes: Vec<&Arquivo> = arquivos.iter().filter(|a| a.tamanho >= NAND_MINIMA).collect();
    grandes.sort_by_key(|a| std::cmp::Reverse(a.tamanho));
    if !grandes.iter().take(10).any(|a| a.ler(0x70).is_ok_and(|cabeca| parece_nand(&cabeca))) {
        return None;
    }
    let nand = escolher_nand(&arquivos).ok()?;
    Some((nand, arquivos))
}

fn mostrar_copias(copias: &[Copia]) {
    if copias.is_empty() {
        dizer!("  Não achei nenhuma cópia da NAND em Downloads, na Área de Trabalho, em Documentos");
        dizer!("  nem nos pendrives. Use a opção [c] para dizer onde ela está.");
    }
    for (n, c) in copias.iter().enumerate() {
        let (tipo, situacao) = c.linhas();
        let nome = c.origem.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        dizer!();
        dizer!("  {} {}", cor(&format!("[{}]", n + 1), "1;36"), cor(&limpo(&nome), "1"));
        dizer!("      em {}", curto(&c.lugar));
        dizer!("      {tipo}");
        dizer!("      {situacao}");
    }
    dizer!();
    dizer!("  {} digitar ou arrastar para cá o caminho de outra cópia (zip, pasta ou arquivo)", cor("[c]", "1;36"));
    dizer!("  {} sair", cor("[s]", "1;36"));
    dizer!();
}

/// O número digitado, se estiver entre 1 e `maximo`.
fn numero(resposta: &str, maximo: usize) -> Option<usize> {
    let n: usize = resposta.parse().ok().filter(|_| resposta.bytes().all(|b| b.is_ascii_digit()))?;
    (1..=maximo).contains(&n).then(|| n - 1)
}

enum Escolha {
    Achada(usize),
    Digitada(Box<Copia>),
}

fn escolher_copia(copias: &[Copia]) -> Result<Option<Escolha>, Falha> {
    loop {
        let resposta = perguntar("Escolha a cópia: ")?.to_lowercase();
        if matches!(resposta.as_str(), "s" | "sair" | "0") {
            return Ok(None);
        }
        if resposta == "c" {
            let caminho = caminho_digitado(&perguntar_bruto("Caminho da cópia: ")?);
            if caminho.as_os_str().is_empty() {
                continue;
            }
            let (nand, arquivos) = match abrir_origem(&caminho) {
                Ok(achado) => achado,
                Err(e) => {
                    dizer!("{}", cor(&format!("  {e}"), "31"));
                    continue;
                }
            };
            let lugar = absoluto(&caminho).parent().map(Path::to_path_buf).unwrap_or_default();
            let mut c = Copia::new(caminho, lugar, nand, arquivos);
            if let Err(e) = c.identificar(None) {
                dizer!("{}", cor(&format!("  não consegui ler a cópia: {e}"), "31"));
                continue;
            }
            return Ok(Some(Escolha::Digitada(Box::new(c))));
        }
        if let Some(n) = numero(&resposta, copias.len()) {
            return Ok(Some(Escolha::Achada(n)));
        }
        dizer!("{}", cor("  Digite o número de uma cópia, c ou s.", "33"));
    }
}

fn avisar_danificado(info: &Achado) {
    dizer!(
        "{}",
        cor(
            &format!(
                "  A CPU key é deste console, mas o KV desta cópia está danificado: o HMAC não confere.{}",
                erro_danificado(info.com_ecc, info.ruins)
            ),
            "31"
        )
    );
    dizer!("  Faça outra cópia da NAND no console. Nada foi gravado.");
}

/// O KV conferido da cópia escolhida; pede a CPU key se ela não estava junto.
fn garantir_kv(c: &mut Copia) -> Result<Option<Achado>, Falha> {
    if let Some(a) = &c.achado {
        return Ok(Some(a.clone()));
    }
    if let Some(d) = &c.danificado {
        avisar_danificado(d);
        return Ok(None);
    }
    if c.tem_chave {
        dizer!("{}", cor("  A CPU key que está junto com esta cópia não abre o KV.", "33"));
    }
    loop {
        let texto = ler_segredo("  Digite a CPU key (32 caracteres, não aparece na tela; ENTER volta): ")?;
        if texto.trim().is_empty() {
            return Ok(None);
        }
        let chaves = chaves_do_texto(texto.as_bytes());
        if chaves.is_empty() {
            dizer!("{}", cor("  Isso não é uma CPU key: ela tem 32 caracteres de 0 a 9 e de A a F.", "33"));
            continue;
        }
        c.identificar(Some(chaves))?;
        if let Some(a) = &c.achado {
            return Ok(Some(a.clone()));
        }
        if let Some(d) = &c.danificado {
            avisar_danificado(d);
            return Ok(None);
        }
        dizer!("{}", cor("  Essa CPU key não abre o KV desta cópia. Confira se ela é deste console.", "33"));
    }
}

/// Só letras, números e traço no nome do arquivo (a série vem do keyvault).
fn para_nome(serie: &str) -> String {
    serie.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect()
}

fn escolher_destino(c: &Copia, kv: &[u8], editores: &[PathBuf]) -> Result<Option<PathBuf>, Falha> {
    let (serie, _) = serie_e_peca(kv);
    let arquivo_kv = format!("KV-{}.bin", para_nome(&serie));
    let pasta_da_copia = absoluto(&c.origem).parent().map(Path::to_path_buf).unwrap_or_default();
    let mut opcoes: Vec<(PathBuf, &str)> =
        editores.iter().map(|p| (p.join("console").join("kv.bin"), "na pasta do editor")).collect();
    opcoes.push((pasta_da_copia.join(&arquivo_kv), "ao lado da cópia"));
    loop {
        dizer!();
        dizer!("{}", cor("  Onde gravar o kv.bin?", "1"));
        for (n, (caminho, rotulo)) in opcoes.iter().enumerate() {
            dizer!("  {} {rotulo}: {}", cor(&format!("[{}]", n + 1), "1;36"), curto(caminho));
        }
        dizer!("  {} na pasta do editor, digitando ou arrastando a pasta dele", cor("[e]", "1;36"));
        dizer!("  {} em outra pasta", cor("[o]", "1;36"));
        dizer!("  {} voltar", cor("[v]", "1;36"));
        let resposta = perguntar("Escolha: ")?.to_lowercase();
        if matches!(resposta.as_str(), "v" | "voltar") {
            return Ok(None);
        }
        if let Some(n) = numero(&resposta, opcoes.len()) {
            return Ok(Some(opcoes.swap_remove(n).0));
        }
        if resposta == "e" || resposta == "o" {
            let pergunta = if resposta == "e" { "Pasta do editor: " } else { "Pasta: " };
            let pasta = caminho_digitado(&perguntar_bruto(pergunta)?);
            if pasta.as_os_str().is_empty() {
                continue;
            }
            if !pasta.is_dir() {
                dizer!("{}", cor(&format!("  Essa pasta não existe: {}", mostrar(&pasta)), "33"));
                continue;
            }
            if resposta == "e" {
                return Ok(Some(pasta.join("console").join("kv.bin")));
            }
            return Ok(Some(pasta.join(&arquivo_kv)));
        }
        dizer!("{}", cor("  Digite o número de uma opção, e, o ou v.", "33"));
    }
}

fn menu(versao: &str) -> Result<(), Falha> {
    ligar_cores();
    titulo(&format!("Extrair o kv.bin do Xbox 360  ·  versão {versao}"));
    dizer!("  A CPU key nunca aparece na tela, e nada vai para a internet.");
    dizer!("  Procurando cópias da NAND em Downloads, Área de Trabalho, Documentos e pendrives...");
    let (mut copias, editores) = procurar();
    loop {
        titulo("Cópias da NAND encontradas");
        mostrar_copias(&copias);
        let mut digitada;
        let c: &mut Copia = match escolher_copia(&copias)? {
            None => return Ok(()),
            Some(Escolha::Achada(n)) => &mut copias[n],
            Some(Escolha::Digitada(c)) => {
                digitada = c;
                &mut digitada
            }
        };
        let Some(info) = garantir_kv(c)? else {
            continue;
        };
        let (serie, peca) = serie_e_peca(&info.kv);
        let peca = if peca.is_empty() { String::new() } else { format!(", peça {peca}") };
        dizer!();
        dizer!("{}", cor(&format!("  CPU key confere. Série {serie}{peca}."), "32"));
        dizer!("  Confira se a série é a mesma da etiqueta do console.");
        while let Some(destino) = escolher_destino(c, &info.kv, &editores)? {
            if let Err(e) = gravar(&destino, &info.kv) {
                dizer!("{}", cor(&format!("  {e}"), "31"));
                continue;
            }
            titulo("Pronto!");
            dizer!("  kv.bin gravado em: {}", curto(&destino));
            dizer!("  SHA-256: {}", sha256_hex(&info.kv));
            dizer!("  Guarde o kv.bin, a cópia da NAND e a CPU key num lugar seguro, com senha.");
            return Ok(());
        }
    }
}

/// Abre o menu e, no fim, espera o ENTER (no Windows, a janela não fecha sozinha).
pub fn abrir(versao: &str) {
    match menu(versao) {
        Ok(()) => {}
        Err(Falha::Cancelado) => dizer!("\nCancelado. Nada foi gravado."),
        Err(Falha::Erro(e)) => dizer!("{}", cor(&format!("\nERRO: {e}"), "31")),
    }
    escrever("\nAperte ENTER para sair.");
    let _ = perguntar_bruto("");
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn reconhece_o_editor() {
        assert!(eh_editor("biohazard5-save-editor"));
        assert!(eh_editor("BIOHAZARD 5 SAVE EDITOR GOLD.exe"));
        assert!(eh_editor("biohazard5-save-editor.exe"));
        assert!(!eh_editor("biohazard5-save-editor.zip"));
        assert!(!eh_editor("biohazard5-save-editor.d"));
        assert!(!eh_editor("extrair-kv"));
        assert!(!eh_editor(".biohazard5-save-editor"));
    }

    #[test]
    fn caminho_com_aspas_escapes_e_til() {
        assert_eq!(caminho_digitado(b"  '/a/b c.zip' "), PathBuf::from("/a/b c.zip"));
        assert_eq!(caminho_digitado(b"\"/a/b c.zip\""), PathBuf::from("/a/b c.zip"));
        if cfg!(unix) {
            assert_eq!(caminho_digitado(br"/a/Backup\ NAND.zip"), PathBuf::from("/a/Backup NAND.zip"));
            if let Some(casa) = casa() {
                assert_eq!(caminho_digitado(b"~/Downloads"), casa.join("Downloads"));
                assert_eq!(caminho_digitado(b"~"), casa);
            }
            assert_eq!(caminho_digitado(b"~x/a"), PathBuf::from("~x/a"));
        }
    }

    #[test]
    fn numero_so_aceita_digitos_no_intervalo() {
        assert_eq!(numero("1", 3), Some(0));
        assert_eq!(numero("3", 3), Some(2));
        assert_eq!(numero("4", 3), None);
        assert_eq!(numero("0", 3), None);
        assert_eq!(numero("+1", 3), None);
        assert_eq!(numero("", 3), None);
    }

    #[test]
    fn nand_pelo_cabecalho() {
        let mut cab = vec![0u8; 0x70];
        cab[..2].copy_from_slice(&MAGIC);
        assert!(!parece_nand(&cab));
        cab[0x60..0x64].copy_from_slice(&0x4000u32.to_be_bytes());
        assert!(parece_nand(&cab));
        assert!(!parece_nand(&cab[..0x6F]));
    }

    #[test]
    fn serie_vira_nome_de_arquivo_seguro() {
        assert_eq!(para_nome("019199654612"), "019199654612");
        assert_eq!(para_nome("../a\\b"), "___a_b");
    }
}
