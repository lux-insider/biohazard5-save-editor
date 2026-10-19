//! O terminal: cores, perguntas e a CPU key digitada sem aparecer na tela.
use std::io::{self, BufRead, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use extrair_kv::Erro;

/// Por que a leitura parou sem resposta.
pub enum Falha {
    Erro(Erro),
    /// Ctrl+C, ou a entrada acabou (Ctrl+D).
    Cancelado,
}

impl From<Erro> for Falha {
    fn from(e: Erro) -> Self {
        Falha::Erro(e)
    }
}

/// Escreve na saída padrão; se ela foi fechada (`| head`), segue sem travar.
macro_rules! dizer {
    () => { $crate::terminal::escrever("\n") };
    ($($t:tt)*) => { $crate::terminal::escrever(&format!("{}\n", format_args!($($t)*))) };
}
pub(crate) use dizer;

pub fn escrever(texto: &str) {
    let mut saida = io::stdout().lock();
    let _ = saida.write_all(texto.as_bytes());
    let _ = saida.flush();
}

/// O mesmo, na saída de erros.
pub fn escrever_erro(texto: &str) {
    let _ = io::stderr().lock().write_all(texto.as_bytes());
}

static CORES: AtomicBool = AtomicBool::new(false);

/// Cores só num terminal de verdade, sem NO_COLOR; no console do Windows,
/// liga o modo que entende as cores.
pub fn ligar_cores() {
    if !io::stdout().is_terminal() || std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return;
    }
    #[cfg(windows)]
    if !windows::ligar_cores() {
        return;
    }
    CORES.store(true, Ordering::Relaxed);
}

pub fn cor(texto: &str, codigo: &str) -> String {
    if CORES.load(Ordering::Relaxed) {
        format!("\x1b[{codigo}m{texto}\x1b[0m")
    } else {
        texto.to_string()
    }
}

/// Mostra a pergunta e devolve a linha digitada, como veio (sem o ENTER).
pub fn perguntar_bruto(texto: &str) -> Result<Vec<u8>, Falha> {
    escrever(&cor(texto, "1"));
    let mut linha = Vec::new();
    match io::stdin().lock().read_until(b'\n', &mut linha) {
        Ok(0) | Err(_) => Err(Falha::Cancelado),
        Ok(_) => {
            while linha.last().is_some_and(|b| matches!(b, b'\n' | b'\r')) {
                linha.pop();
            }
            Ok(linha)
        }
    }
}

/// Mostra a pergunta e devolve a resposta sem os espaços das pontas.
pub fn perguntar(texto: &str) -> Result<String, Falha> {
    Ok(String::from_utf8_lossy(&perguntar_bruto(texto)?).trim().to_string())
}

/// Tamanho máximo do que é digitado no lugar da CPU key.
const SEGREDO_MAXIMO: usize = 4096;

/// Pede a CPU key sem mostrar o que é digitado. Ctrl+C cancela.
pub fn ler_segredo(pergunta: &str) -> Result<String, Falha> {
    #[cfg(unix)]
    return unix::ler_segredo(pergunta);
    #[cfg(windows)]
    return windows::ler_segredo(pergunta);
}

#[cfg(unix)]
mod unix {
    use std::fs::{File, OpenOptions};
    use std::io::{self, Read, Write};

    use extrair_kv::{motivo, Erro};
    use rustix::termios::{tcgetattr, tcsetattr, LocalModes, OptionalActions, SpecialCodeIndex};

    use super::{Falha, SEGREDO_MAXIMO};

    /// Como o getpass do Python: no próprio terminal (/dev/tty), sem eco.
    /// O Ctrl+C chega como um caractere (ISIG desligado), para o terminal
    /// voltar ao normal antes de cancelar.
    pub fn ler_segredo(pergunta: &str) -> Result<String, Falha> {
        let erro = |e: io::Error| Falha::Erro(Erro(format!("não consegui ler a CPU key do terminal: {}", motivo(&e))));
        let mut tty = OpenOptions::new().read(true).write(true).open("/dev/tty").map_err(erro)?;
        let original = tcgetattr(&tty).map_err(|e| erro(e.into()))?;
        let mut sem_eco = original.clone();
        sem_eco.local_modes.remove(LocalModes::ECHO | LocalModes::ICANON | LocalModes::ISIG);
        sem_eco.special_codes[SpecialCodeIndex::VMIN] = 1;
        sem_eco.special_codes[SpecialCodeIndex::VTIME] = 0;
        tcsetattr(&tty, OptionalActions::Flush, &sem_eco).map_err(|e| erro(e.into()))?;
        let lido = tty.write_all(pergunta.as_bytes()).and_then(|()| tty.flush()).and_then(|()| ler_linha(&mut tty));
        let _ = tcsetattr(&tty, OptionalActions::Flush, &original);
        let _ = tty.write_all(b"\n");
        match lido {
            Ok(Some(texto)) => Ok(texto),
            Ok(None) => Err(Falha::Cancelado),
            Err(e) => Err(erro(e)),
        }
    }

    /// Lê até o ENTER. `None`: Ctrl+C, ou Ctrl+D com a linha vazia.
    fn ler_linha(tty: &mut File) -> io::Result<Option<String>> {
        let mut texto: Vec<u8> = Vec::new();
        let mut b = [0u8; 1];
        loop {
            match tty.read(&mut b) {
                Ok(0) => return Ok(None),
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
            match b[0] {
                b'\r' | b'\n' => break,
                0x03 => return Ok(None),
                0x04 if texto.is_empty() => return Ok(None),
                0x7F | 0x08 => {
                    // apaga um caractere inteiro, mesmo acentuado
                    while texto.pop().is_some_and(|c| c & 0xC0 == 0x80) {}
                }
                0x15 => texto.clear(),
                c if texto.len() < SEGREDO_MAXIMO => texto.push(c),
                _ => {}
            }
        }
        Ok(Some(String::from_utf8_lossy(&texto).into_owned()))
    }
}

/// No Windows: o console precisa de duas funções do kernel32 para as cores
/// e do _getwch do runtime do C para ler tecla por tecla, sem eco. São as
/// únicas linhas com `unsafe` do programa.
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows {
    use std::ffi::c_void;
    use std::io::{self, IsTerminal};
    use std::os::windows::io::AsRawHandle;

    use super::{escrever, escrever_erro, Falha, SEGREDO_MAXIMO};

    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    const WEOF: u16 = 0xFFFF;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetConsoleMode(console: *mut c_void, modo: *mut u32) -> i32;
        fn SetConsoleMode(console: *mut c_void, modo: u32) -> i32;
    }

    extern "C" {
        fn _getwch() -> u16;
    }

    pub fn ligar_cores() -> bool {
        let saida = io::stdout().as_raw_handle();
        let mut modo = 0u32;
        // SAFETY: `saida` é o handle da saída padrão deste processo, válido
        // enquanto o programa roda; `modo` é uma variável local.
        unsafe { GetConsoleMode(saida, &mut modo) != 0 && SetConsoleMode(saida, modo | ENABLE_VIRTUAL_TERMINAL_PROCESSING) != 0 }
    }

    /// Na tela, mesmo com a saída padrão indo para um arquivo.
    fn na_tela(texto: &str) {
        if io::stdout().is_terminal() {
            escrever(texto);
        } else {
            escrever_erro(texto);
        }
    }

    /// Como o getpass do Python no Windows: tecla por tecla, com o _getwch,
    /// que não mostra nada e entrega o Ctrl+C como um caractere.
    pub fn ler_segredo(pergunta: &str) -> Result<String, Falha> {
        na_tela(pergunta);
        let mut texto: Vec<u16> = Vec::new();
        loop {
            // SAFETY: _getwch não recebe nada; devolve a próxima tecla do console.
            let c = unsafe { _getwch() };
            match c {
                0x0D | 0x0A => break,
                0x03 | WEOF => {
                    na_tela("\r\n");
                    return Err(Falha::Cancelado);
                }
                0x08 => {
                    texto.pop();
                }
                c if texto.len() < SEGREDO_MAXIMO => texto.push(c),
                _ => {}
            }
        }
        na_tela("\r\n");
        Ok(String::from_utf16_lossy(&texto))
    }
}
