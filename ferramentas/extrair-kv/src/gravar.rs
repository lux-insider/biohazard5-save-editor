//! Gravar o kv.bin sem perder nenhum outro.
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use crate::erro::{motivo, Erro};
use crate::texto::mostrar;

/// Grava sem nunca sobrescrever, só para o dono, e confere o que foi gravado.
///
/// A pasta é criada se não existir (por exemplo, `console/kv.bin`). Se algo
/// der errado depois de criar o arquivo, ele é apagado: ou o kv.bin fica
/// inteiro e conferido, ou não fica nada.
pub fn gravar(destino: &Path, kv: &[u8]) -> Result<(), Erro> {
    let ja_existe = || Erro(format!("já existe {}. Mova ou renomeie esse arquivo antes, para não perder nenhum KV.", mostrar(destino)));
    let nao_criou = |e: io::Error| Erro(format!("não consegui criar {}: {}", mostrar(destino), motivo(&e)));
    if let Some(pasta) = destino.parent().filter(|p| !p.as_os_str().is_empty() && !p.is_dir()) {
        criar_pasta(pasta).map_err(|e| if e.kind() == io::ErrorKind::AlreadyExists { ja_existe() } else { nao_criou(e) })?;
    }
    let mut f = abrir_novo(destino).map_err(|e| if e.kind() == io::ErrorKind::AlreadyExists { ja_existe() } else { nao_criou(e) })?;
    let escrito = f.write_all(kv).and_then(|()| f.sync_all());
    drop(f);
    let gravado = match escrito {
        Err(e) => Err(Erro(format!("não consegui gravar {}: {}", mostrar(destino), motivo(&e)))),
        Ok(()) => match fs::read(destino) {
            Ok(lido) if lido == kv => Ok(()),
            Ok(_) => Err(Erro::new("o arquivo gravado não ficou igual ao keyvault. Confira o disco.")),
            Err(e) => Err(Erro(format!("não consegui conferir {}: {}", mostrar(destino), motivo(&e)))),
        },
    };
    if gravado.is_err() {
        let _ = fs::remove_file(destino);
    }
    gravado
}

#[cfg(unix)]
fn criar_pasta(pasta: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().recursive(true).mode(0o700).create(pasta)
}

#[cfg(not(unix))]
fn criar_pasta(pasta: &Path) -> io::Result<()> {
    fs::create_dir_all(pasta)
}

/// Cria o arquivo só se ele não existe (nem como atalho), com permissão 600.
fn abrir_novo(destino: &Path) -> io::Result<fs::File> {
    let mut opcoes = OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opcoes.mode(0o600);
    }
    opcoes.open(destino)
}
