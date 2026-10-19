//! Textos da Biblioteca e documentos do jogo (src/dados/historia.json).
//!
//! 12 arquivos da Biblioteca e 26 documentos das fases (tabela de textos do
//! executável) e 9 documentos das DLCs. Só leitura: nada disso vai para o save.

use serde::Deserialize;

const HISTORIA_JSON: &str = include_str!("dados/historia.json");

#[derive(Clone, Debug, Deserialize)]
pub struct Texto {
    /// "biblioteca", "documento" ou "dlc"
    pub grupo: String,
    /// Número na Biblioteca (1–12) ou na tabela de documentos; nenhum nas DLCs.
    pub numero: Option<u32>,
    /// Título e páginas originais (inglês, como no jogo). As páginas originais
    /// ficam nos dados para conferir a tradução (testes).
    pub titulo: String,
    #[allow(dead_code)]
    pub paginas: Vec<String>,
    /// Tradução do projeto (ferramentas/trad), página a página.
    pub titulo_pt: String,
    pub paginas_pt: Vec<String>,
}

pub fn carrega() -> Vec<Texto> {
    #[derive(Deserialize)]
    struct Arquivo {
        textos: Vec<Texto>,
    }
    serde_json::from_str::<Arquivo>(HISTORIA_JSON).expect("src/dados/historia.json inválido").textos
}

/// Posição do arquivo da Biblioteca (1–12) na lista de desbloqueios
/// "arquivos" do save, que vem na ordem inversa (Albert Wesker primeiro).
pub fn indice_desbloqueio(numero: u32) -> Option<usize> {
    (1..=12).contains(&numero).then(|| 12 - numero as usize)
}

/// Ilustração da Biblioteca do jogo para o arquivo (ui/imagens/biblioteca,
/// na ordem dos arquivos 1–12; ViewerResource tm120, tm121 e tm122): a
/// posição na lista ARTES de ui/telas/historia.slint; -1 = sem ilustração.
pub fn arte(t: &Texto) -> i32 {
    match t.numero {
        Some(n @ 1..=12) if t.grupo == "biblioteca" => n as i32 - 1,
        _ => -1,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn textos_completos() {
        let t = carrega();
        let n = |g: &str| t.iter().filter(|x| x.grupo == g).count();
        assert_eq!((n("biblioteca"), n("documento"), n("dlc")), (12, 26, 9));
        assert!(t.iter().all(|x| !x.titulo.is_empty() && !x.paginas.is_empty()));
        // toda tradução tem as mesmas páginas do original
        assert!(t.iter().all(|x| !x.titulo_pt.is_empty() && x.paginas_pt.len() == x.paginas.len()));
        assert_eq!(t[0].titulo_pt, "A História de RESIDENT EVIL");
        assert_eq!(t[0].titulo, "History of RESIDENT EVIL");
        assert_eq!(t[11].titulo, "Albert Wesker");
    }

    #[test]
    fn ilustracoes_nos_arquivos_certos() {
        let t = carrega();
        let de = |titulo: &str| t.iter().find(|x| x.grupo == "biblioteca" && x.titulo == titulo).map(arte);
        // mesma ordem dos arquivos em ui/telas/historia.slint
        let artes = [
            "History of RESIDENT EVIL", "BSAA", "Majini", "Chris Redfield", "Sheva Alomar", "Ricardo Irving",
            "Ndipaya Tribe", "U-8", "Tricell", "Jill Valentine", "Excella Gionne", "Albert Wesker",
        ];
        for (i, titulo) in artes.iter().enumerate() {
            assert_eq!(de(titulo), Some(i as i32), "{titulo}");
        }
        assert_eq!(t.iter().filter(|x| arte(x) >= 0).count(), 12);
    }

    #[test]
    fn biblioteca_na_ordem_do_save() {
        assert_eq!(indice_desbloqueio(12), Some(0)); // Albert Wesker
        assert_eq!(indice_desbloqueio(1), Some(11)); // A Saga de Resident Evil
        assert_eq!(indice_desbloqueio(13), None);
    }
}
