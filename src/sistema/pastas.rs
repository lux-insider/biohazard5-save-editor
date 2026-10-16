//! Onde ficam o keyvault, os backups e por onde os diálogos começam.
use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| ".".into())
}

pub fn pasta_do_exe() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| ".".into())
}

/// De onde vem a chave que assina o save do Xbox.
pub enum Chave {
    Arquivo(PathBuf),
    /// kv.bin embutido na versão pessoal (feature `kv-embutido`).
    Embutida,
    /// Chave interna do programa (sempre existe).
    Interna,
}

impl Chave {
    pub fn carregar(&self) -> Result<crate::save::ChaveConsole, String> {
        match self {
            Chave::Arquivo(p) => crate::save::ChaveConsole::carrega(p),
            Chave::Embutida => crate::save::ChaveConsole::de_bytes(super::chave_embutida::KV),
            Chave::Interna => {
                use super::chave_interna as c;
                crate::save::ChaveConsole::de_componentes(&c::CERT, &c::N, &c::P, &c::Q)
            }
        }
    }

    /// Como a chave aparece na nota da tela.
    pub fn descricao(&self) -> String {
        match self {
            Chave::Arquivo(p) => format!("o keyvault {}", p.display()),
            Chave::Embutida => match self.carregar() {
                Ok(k) => format!("a chave embutida no programa (console {})", k.console),
                Err(e) => format!("a chave embutida no programa ({e})"),
            },
            Chave::Interna => match self.carregar() {
                Ok(k) => format!("a chave interna do programa (console {})", k.console),
                Err(e) => format!("a chave interna do programa ({e})"),
            },
        }
    }
}

/// Chave que assina o save do Xbox. `console/kv.bin` ao lado do programa
/// sempre tem prioridade; depois vem o kv.bin embutido (feature
/// `kv-embutido`) e, por último, a chave interna do programa, que
/// existe sempre: o save do Xbox sempre sai assinado.
pub fn chave() -> Option<Chave> {
    let local = pasta_do_exe().join("console").join("kv.bin");
    if local.is_file() {
        return Some(Chave::Arquivo(local));
    }
    if super::chave_embutida::EXISTE {
        return Some(Chave::Embutida);
    }
    Some(Chave::Interna)
}

/// Backups ao lado do programa; se a pasta não for gravável, na pasta de
/// dados do usuário.
pub fn pasta_backups() -> Result<PathBuf, String> {
    let local = pasta_do_exe().join("backups");
    if std::fs::create_dir_all(&local).is_ok() && gravavel(&local) {
        return Ok(local);
    }
    let dados = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local").join("share"))
        // Nome antigo da pasta, mantido para achar os backups das versões anteriores.
        .join("re5-save-editor-360-pc")
        .join("backups");
    std::fs::create_dir_all(&dados).map_err(|e| format!("backup: {e}"))?;
    Ok(dados)
}

/// Pasta do perfil ao lado do save: o save fica em
/// `Content\<XUID>\434307D4\00000001\savedata.bin` e o perfil em
/// `Content\<XUID>\FFFE07D1\00010000\`. Só se a pasta existir.
pub fn pasta_perfil(save: Option<&Path>, xuid: &str) -> Option<PathBuf> {
    let content = save?.parent()?.parent()?.parent()?.parent()?;
    let p = content.join(xuid).join("FFFE07D1").join("00010000");
    p.is_dir().then_some(p)
}

/// Pasta de dados do usuário (a mesma da reserva dos backups).
fn pasta_dados() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local").join("share"))
        .join("re5-save-editor-360-pc")
}

/// Gamertags já lidas de perfis: XUID (16 hex) -> gamertag. Só isso é guardado.
fn arquivo_gamertags() -> PathBuf {
    pasta_dados().join("gamertags.json")
}

pub fn carregar_gamertags() -> std::collections::HashMap<String, String> {
    std::fs::read_to_string(arquivo_gamertags())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn guardar_gamertags(g: &std::collections::HashMap<String, String>) -> Result<(), String> {
    let caminho = arquivo_gamertags();
    if let Some(p) = caminho.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("gamertags: {e}"))?;
    }
    let texto = serde_json::to_string_pretty(g).map_err(|e| e.to_string())?;
    std::fs::write(&caminho, texto).map_err(|e| format!("gamertags: {e}"))
}

fn gravavel(p: &Path) -> bool {
    let t = p.join(".teste-escrita");
    let ok = std::fs::write(&t, b"").is_ok();
    let _ = std::fs::remove_file(&t);
    ok
}

/// Onde os diálogos começam: a pasta do save aberto ou um lugar provável.
pub fn pasta_inicial(atual: Option<&Path>) -> PathBuf {
    if let Some(p) = atual.and_then(Path::parent) {
        return p.into();
    }
    let h = home();
    // O save de PC fica na pasta da Steam: userdata/<conta>/21690/remote.
    // Com uma conta só, o diálogo já abre na pasta do save.
    if let Some(p) = pasta_save_steam(&h) {
        return p;
    }
    let candidatos = [
        h.join("Documentos").join("434307D4").join("00000001"),
        h.join("Documents").join("434307D4").join("00000001"),
    ];
    candidatos.into_iter().find(|p| p.is_dir()).unwrap_or(h)
}

/// ID do RE5 na Steam.
const APP_STEAM: &str = "21690";

/// Pastas `userdata` da Steam: Windows (Program Files, com e sem x86),
/// Linux (nativo, Flatpak e Snap).
fn userdata_steam(h: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = ["ProgramFiles(x86)", "ProgramFiles"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|p| PathBuf::from(p).join("Steam").join("userdata"))
        .collect();
    v.push(PathBuf::from(r"C:\Program Files (x86)\Steam\userdata"));
    v.push(h.join(".steam").join("steam").join("userdata"));
    v.push(h.join(".local").join("share").join("Steam").join("userdata"));
    v.push(h.join(".var").join("app").join("com.valvesoftware.Steam").join(".local").join("share").join("Steam").join("userdata"));
    v.push(h.join("snap").join("steam").join("common").join(".local").join("share").join("Steam").join("userdata"));
    v
}

/// A pasta do save do RE5 na Steam; com várias contas, a `userdata`.
fn pasta_save_steam(h: &Path) -> Option<PathBuf> {
    let userdata = userdata_steam(h).into_iter().find(|p| p.is_dir())?;
    let saves: Vec<PathBuf> = std::fs::read_dir(&userdata)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path().join(APP_STEAM).join("remote")))
        .filter(|p| p.is_dir())
        .collect();
    match saves.as_slice() {
        [um] => Some(um.clone()),
        _ => Some(userdata),
    }
}
