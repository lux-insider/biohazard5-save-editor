//! extrair-kv: tira o kv.bin (keyvault) da cópia da NAND do Xbox 360.
//!
//! Sem argumentos, num terminal, abre um menu que acha as cópias sozinho;
//! com argumentos, faz tudo de uma vez (ver AJUDA).
#![deny(unsafe_code)]

mod menu;
mod terminal;

use std::ffi::OsString;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use extrair_kv::{
    abrir_kv, abrir_origem, absoluto, chaves_do_texto, chaves_dos_arquivos, descrever, gravar, limpo, mostrar,
    serie_e_peca, sem_repetir, sha256_hex, CpuKey, Erro, CHAVES_MAXIMO, KV_TAMANHO, LEITURA, MAGIC, NAND_MINIMA, PAGINA,
    TEXTO_MAXIMO,
};
use terminal::{dizer, Falha};

const VERSAO: &str = "3.0";

const USO: &str = "uso: extrair-kv [ORIGEM [CPUKEY]] [-o ARQUIVO] [--gravar-danificado]";

const AJUDA: &str = "\
Extrai o KV.bin (keyvault) de uma cópia da NAND do Xbox 360.

Funciona com a cópia de qualquer placa, de qualquer tamanho:
  - NAND de 16, 64, 256 ou 512 MB, com ou sem os 16 bytes de ECC de cada
    página (Xenon, Zephyr, Falcon, Jasper, Trinity, Corona 16 MB);
  - eMMC de 4 GB, na cópia de 48 MB ou na imagem inteira (Corona 4 GB,
    Winchester).

Lê as cópias do Simple 360 NAND Flasher (flashdmp.bin, recovery.bin,
cpukey.txt e o log) e do J-Runner (nanddump*.bin e cpukey.txt).

Uso:
  extrair-kv                            abre um menu que acha as cópias sozinho
                                        (no Windows: dois cliques no extrair-kv.exe)
  extrair-kv \"Backup NAND.zip\"          lê direto do zip
  extrair-kv pasta-do-backup            procura na pasta
  extrair-kv flashdmp.bin cpukey.txt    arquivos soltos
  extrair-kv flashdmp.bin               usa o cpukey.txt da mesma pasta,
                                        ou pede a CPU key, sem mostrar

Opções:
  -o ARQUIVO              onde gravar (padrão: KV.bin na pasta atual; para a
                          versão normal do editor, use -o console/kv.bin)
  --gravar-danificado     grava mesmo se o KV estiver danificado
  -h, --help              mostra esta ajuda
  --version               mostra a versão

O KV só é gravado depois de conferido do mesmo jeito que o console confere:
o HMAC-SHA1 da CPU key tem que bater com o do keyvault. Tudo acontece neste
computador, nada vai para a internet, e a CPU key nunca aparece na tela.";

struct Argumentos {
    origem: Option<OsString>,
    cpukey: Option<OsString>,
    saida: OsString,
    gravar_danificado: bool,
}

enum Pedido {
    Executar(Argumentos),
    Ajuda,
    Versao,
}

const LONGAS: [&str; 4] = ["--saida", "--gravar-danificado", "--help", "--version"];

/// A opção longa pelo nome inteiro ou por um começo que só ela tem (como o argparse).
fn opcao_longa(nome: &str) -> Result<&'static str, String> {
    if let Some(exata) = LONGAS.iter().find(|o| **o == nome) {
        return Ok(exata);
    }
    let parecidas: Vec<&str> = LONGAS.iter().copied().filter(|o| o.starts_with(nome)).collect();
    match parecidas[..] {
        [uma] => Ok(uma),
        [] => Err(format!("opção desconhecida: {nome}")),
        _ => Err(format!("opção ambígua: {nome} ({})", parecidas.join(", "))),
    }
}

fn ler_argumentos(args: impl IntoIterator<Item = OsString>) -> Result<Pedido, String> {
    let mut posicionais = Vec::new();
    let mut saida = OsString::from("KV.bin");
    let mut gravar_danificado = false;
    let mut so_posicionais = false;
    let mut args = args.into_iter();
    let falta_arquivo = || "a opção -o/--saida precisa do nome do arquivo".to_string();
    while let Some(arg) = args.next() {
        // um caminho que não é UTF-8 fica como veio (nunca é uma opção)
        let texto = arg.to_str().unwrap_or("");
        if so_posicionais || !texto.starts_with('-') || texto == "-" {
            posicionais.push(arg);
            continue;
        }
        if texto == "--" {
            so_posicionais = true;
            continue;
        }
        if let Some(longa) = texto.strip_prefix("--") {
            let (nome, valor) = match longa.split_once('=') {
                Some((n, v)) => (format!("--{n}"), Some(v)),
                None => (texto.to_string(), None),
            };
            match opcao_longa(&nome)? {
                "--saida" => saida = valor.map(OsString::from).or_else(|| args.next()).ok_or_else(falta_arquivo)?,
                "--gravar-danificado" if valor.is_none() => gravar_danificado = true,
                "--help" if valor.is_none() => return Ok(Pedido::Ajuda),
                "--version" if valor.is_none() => return Ok(Pedido::Versao),
                outra => return Err(format!("a opção {outra} não leva valor")),
            }
            continue;
        }
        match texto.as_bytes().get(1) {
            Some(b'o') if texto.len() > 2 => saida = OsString::from(&texto[2..]),
            Some(b'o') => saida = args.next().ok_or_else(falta_arquivo)?,
            Some(b'h') if texto.len() == 2 => return Ok(Pedido::Ajuda),
            _ => return Err(format!("opção desconhecida: {}", arg.to_string_lossy())),
        }
    }
    if posicionais.len() > 2 {
        let sobra: Vec<String> = posicionais[2..].iter().map(|a| a.to_string_lossy().into_owned()).collect();
        return Err(format!("argumentos a mais: {}", sobra.join(" ")));
    }
    let mut posicionais = posicionais.into_iter();
    Ok(Pedido::Executar(Argumentos {
        origem: posicionais.next(),
        cpukey: posicionais.next(),
        saida,
        gravar_danificado,
    }))
}

fn mb(tamanho: u64) -> String {
    format!("{:.0}", tamanho as f64 / (1u64 << 20) as f64)
}

fn pedir_chave() -> Result<Vec<CpuKey>, Falha> {
    if !io::stdin().is_terminal() {
        return Err(Erro::new("não achei a CPU key. Passe o arquivo dela depois da NAND: extrair-kv NAND cpukey.txt").into());
    }
    let texto = terminal::ler_segredo("CPU key (32 caracteres, não aparece na tela): ")?;
    let chaves = chaves_do_texto(texto.as_bytes());
    if chaves.is_empty() {
        return Err(Erro::new("isso não é uma CPU key: ela tem 32 caracteres de 0 a 9 e de A a F.").into());
    }
    Ok(chaves)
}

/// O texto do erro quando o KV não pode ser gravado.
fn erro_danificado(com_ecc: bool, ruins: usize) -> String {
    if com_ecc && ruins > 0 {
        format!(" {ruins} página(s) do KV estão com o ECC errado.")
    } else {
        String::new()
    }
}

fn executar(a: Argumentos) -> Result<(), Falha> {
    let origem = PathBuf::from(a.origem.unwrap_or_default());
    let (nand, arquivos) = abrir_origem(&origem)?;

    let chaves = match &a.cpukey {
        Some(arquivo) => {
            let arquivo = Path::new(arquivo);
            if !arquivo.is_file() {
                return Err(Erro(format!("não achei o arquivo da CPU key: {}", mostrar(arquivo))).into());
            }
            let chaves = chaves_do_texto(&extrair_kv::do_disco(arquivo, 0).ler(TEXTO_MAXIMO)?);
            if chaves.is_empty() {
                return Err(
                    Erro(format!("não achei uma CPU key (32 caracteres hexadecimais) em {}.", mostrar(arquivo))).into()
                );
            }
            chaves
        }
        None => match chaves_dos_arquivos(&arquivos, &nand)? {
            chaves if chaves.is_empty() => pedir_chave()?,
            chaves => chaves,
        },
    };
    let chaves = sem_repetir(chaves, CHAVES_MAXIMO);
    let mut saida = PathBuf::from(&a.saida);
    if saida.is_dir() {
        saida.push("KV.bin");
    }

    dizer!("NAND:      {} ({} MB)", limpo(&nand.nome), mb(nand.tamanho));
    let prefixo = nand.ler(LEITURA)?;
    if (prefixo.len() as u64) < NAND_MINIMA {
        return Err(Erro(format!("a cópia é pequena demais para ter o keyvault ({} bytes).", prefixo.len())).into());
    }
    if prefixo[..2] != MAGIC {
        dizer!("AVISO:     a cópia não começa com FF 4F, como a NAND de um console comum. Confira o arquivo.");
    }

    let (achado, danificado) = abrir_kv(&prefixo, nand.tamanho, &chaves);
    let info = match (&achado, danificado) {
        (Some(a), _) => a.clone(),
        (None, Some(d)) if a.gravar_danificado => d,
        (None, Some(d)) => {
            return Err(Erro(format!(
                "a CPU key é deste console, mas o keyvault da cópia está danificado: o HMAC não confere.{} \
                 Faça outra cópia da NAND. Nada foi gravado.",
                erro_danificado(d.com_ecc, d.ruins)
            ))
            .into())
        }
        (None, None) => {
            let quais = if chaves.len() == 1 {
                "a CPU key".to_string()
            } else {
                format!("nenhuma das {} CPU keys encontradas", chaves.len())
            };
            return Err(Erro(format!(
                "o keyvault não abriu com {quais}. Ou a CPU key não é deste console, ou a cópia não é \
                 uma NAND do Xbox 360. Nada foi gravado."
            ))
            .into());
        }
    };

    dizer!("Tipo:      {}", descrever(nand.tamanho, info.com_ecc));
    let de_onde = if info.do_cabecalho { "lida do cabeçalho da NAND" } else { "a padrão (a do cabeçalho não servia)" };
    dizer!("Keyvault:  posição 0x{:X}, {de_onde}", info.inicio);
    if info.com_ecc {
        let paginas = KV_TAMANHO / PAGINA;
        dizer!("ECC:       {} de {paginas} páginas do KV conferem", paginas - info.ruins.min(paginas));
    }
    if achado.is_some() {
        dizer!("CPU key:   confere (HMAC-SHA1 igual ao que o console calcula)");
    } else {
        dizer!("AVISO:     o HMAC não confere: o KV está danificado. Gravado só porque você pediu.");
    }
    let (serie, peca) = serie_e_peca(&info.kv);
    dizer!("Série:     {serie}   <- tem que ser igual ao da etiqueta do console");
    if !peca.is_empty() {
        dizer!("Peça:      {peca}");
    }

    gravar(&saida, &info.kv)?;
    let dono = if cfg!(unix) { ", só você pode ler" } else { "" };
    dizer!("Gravado:   {} ({} bytes){dono}", mostrar(&absoluto(&saida)), info.kv.len());
    dizer!("SHA-256:   {}", sha256_hex(&info.kv));
    Ok(())
}

fn main() -> ExitCode {
    let pedido = match ler_argumentos(std::env::args_os().skip(1)) {
        Ok(p) => p,
        Err(mensagem) => {
            terminal::escrever_erro(&format!("{USO}\nextrair-kv: erro: {mensagem}\n"));
            return ExitCode::from(2);
        }
    };
    let argumentos = match pedido {
        Pedido::Ajuda => {
            dizer!("{AJUDA}");
            return ExitCode::SUCCESS;
        }
        Pedido::Versao => {
            dizer!("extrair-kv {VERSAO}");
            return ExitCode::SUCCESS;
        }
        Pedido::Executar(a) => a,
    };
    if argumentos.origem.is_none() {
        if !io::stdin().is_terminal() {
            dizer!("{AJUDA}");
            return ExitCode::from(2);
        }
        menu::abrir(VERSAO);
        return ExitCode::SUCCESS;
    }
    match executar(argumentos) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Falha::Erro(e)) => {
            terminal::escrever_erro(&format!("ERRO: {e}\n"));
            ExitCode::from(1)
        }
        Err(Falha::Cancelado) => {
            terminal::escrever_erro("\nCancelado. Nada foi gravado.\n");
            ExitCode::from(130)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn ler(args: &[&str]) -> Result<Pedido, String> {
        ler_argumentos(args.iter().map(OsString::from))
    }

    fn executar_com(args: &[&str]) -> Argumentos {
        match ler(args) {
            Ok(Pedido::Executar(a)) => a,
            _ => panic!("esperava executar: {args:?}"),
        }
    }

    #[test]
    fn argumentos_como_o_argparse() {
        let a = executar_com(&["nand.bin", "cpukey.txt", "-o", "console/kv.bin", "--gravar-danificado"]);
        assert_eq!(a.origem.as_deref(), Some("nand.bin".as_ref()));
        assert_eq!(a.cpukey.as_deref(), Some("cpukey.txt".as_ref()));
        assert_eq!(a.saida, "console/kv.bin");
        assert!(a.gravar_danificado);
        assert_eq!(executar_com(&["x", "--saida=y"]).saida, "y");
        assert_eq!(executar_com(&["x", "-oy"]).saida, "y");
        assert_eq!(executar_com(&["x", "--sai", "y"]).saida, "y");
        assert!(executar_com(&["x", "--gravar"]).gravar_danificado);
        assert_eq!(executar_com(&["--", "-nand.bin"]).origem.as_deref(), Some("-nand.bin".as_ref()));
        assert_eq!(executar_com(&[]).saida, "KV.bin");
        assert!(matches!(ler(&["-h"]), Ok(Pedido::Ajuda)));
        assert!(matches!(ler(&["x", "--help"]), Ok(Pedido::Ajuda)));
        assert!(matches!(ler(&["--version"]), Ok(Pedido::Versao)));
    }

    #[test]
    fn argumentos_errados() {
        assert!(ler(&["a", "b", "c"]).is_err());
        assert!(ler(&["-x"]).is_err());
        assert!(ler(&["--nada"]).is_err());
        assert!(ler(&["a", "-o"]).is_err());
        assert!(ler(&["--gravar-danificado=sim"]).is_err());
    }
}
