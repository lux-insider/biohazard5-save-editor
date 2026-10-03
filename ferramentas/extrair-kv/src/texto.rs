//! Nomes e caminhos para mostrar na tela.
use std::path::{Component, Path, PathBuf};

/// Nome para mostrar na tela, sem caracteres de controle nem os invisíveis
/// que mudam a direção do texto (um nome dentro de um zip pode ter qualquer coisa).
pub fn limpo(nome: &str) -> String {
    nome.chars().map(|c| if visivel(c) { c } else { '?' }).collect()
}

fn visivel(c: char) -> bool {
    !(c.is_control()
        || matches!(c,
            '\u{00A0}' | '\u{00AD}' | '\u{061C}' | '\u{1680}' | '\u{180E}'
            | '\u{2000}'..='\u{200F}' | '\u{2028}'..='\u{202F}' | '\u{205F}'..='\u{206F}'
            | '\u{3000}' | '\u{FEFF}' | '\u{FFF9}'..='\u{FFFB}'))
}

/// Caminho para mostrar na tela.
pub fn mostrar(caminho: &Path) -> String {
    limpo(&caminho.to_string_lossy())
}

/// Caminho absoluto, com `.` e `..` resolvidos sem seguir atalhos (como o
/// os.path.abspath do Python).
pub fn absoluto(caminho: &Path) -> PathBuf {
    let completo = if caminho.is_absolute() {
        caminho.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(caminho)
    };
    let mut saida = PathBuf::new();
    for parte in completo.components() {
        match parte {
            Component::CurDir => {}
            Component::ParentDir => {
                saida.pop();
            }
            outra => saida.push(outra),
        }
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn limpo_troca_controles_e_invisiveis() {
        assert_eq!(limpo("flashdmp.bin"), "flashdmp.bin");
        assert_eq!(limpo("Área de Trabalho"), "Área de Trabalho");
        assert_eq!(limpo("a\x1b[31mb\n"), "a?[31mb?");
        assert_eq!(limpo("nib.exe\u{202E}"), "nib.exe?");
    }

    #[cfg(unix)]
    #[test]
    fn absoluto_resolve_pontos() {
        assert_eq!(absoluto(Path::new("/a/b/../c/./d")), PathBuf::from("/a/c/d"));
        assert_eq!(absoluto(Path::new("/..")), PathBuf::from("/"));
        assert!(absoluto(Path::new("x")).is_absolute());
    }
}
