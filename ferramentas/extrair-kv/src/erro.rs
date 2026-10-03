//! Erros com mensagem para o usuário, já em português.
use std::fmt;
use std::io;

/// Erro com mensagem pronta para mostrar na tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Erro(pub String);

impl Erro {
    pub fn new(mensagem: impl Into<String>) -> Self {
        Erro(mensagem.into())
    }
}

impl fmt::Display for Erro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Erro {}

/// O motivo de um erro de leitura ou gravação, em português quando é um dos
/// comuns; nos outros, o texto do sistema sem o "(os error N)" do fim.
pub fn motivo(e: &io::Error) -> String {
    use io::ErrorKind as K;
    let texto = match e.kind() {
        K::NotFound => "não existe",
        K::PermissionDenied => "sem permissão",
        K::AlreadyExists => "já existe",
        K::StorageFull => "o disco está cheio",
        K::ReadOnlyFilesystem => "o disco é só de leitura",
        K::IsADirectory => "é uma pasta, não um arquivo",
        K::NotADirectory => "uma parte do caminho não é pasta",
        K::UnexpectedEof => "o arquivo acabou antes do esperado",
        _ => {
            let mut s = e.to_string();
            if let Some(i) = s.rfind(" (os error ") {
                s.truncate(i);
            }
            return s;
        }
    };
    texto.to_string()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn motivo_em_portugues_e_sem_codigo() {
        assert_eq!(motivo(&io::Error::from(io::ErrorKind::PermissionDenied)), "sem permissão");
        assert_eq!(motivo(&io::Error::other("deu ruim")), "deu ruim");
        let e = io::Error::from_raw_os_error(5);
        assert!(!motivo(&e).contains("os error"), "{}", motivo(&e));
    }
}
