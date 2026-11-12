//! Extratores de COBOL, Delphi e Visual Basic 6 no mapa do "Faturamento Exemplo"
//! e em fontes em Windows-1252.

use std::fs;
use std::path::{Path, PathBuf};

use cba_index::{
    Linhas, Mapa, Projeto, Registro, TipoReferencia, TipoSimbolo, inventariar, mapear,
};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn mapa_de(raiz: &Path) -> Mapa {
    let projeto = Projeto::novo("Legado", [raiz]).unwrap();
    let inv = inventariar(&projeto, None).unwrap();
    mapear(&inv, &projeto.escopo().unwrap(), None, &Registro::padrao())
}

/// Único símbolo com o nome, fora das declarações.
fn um<'a>(mapa: &'a Mapa, nome: &str, tipo: TipoSimbolo) -> cba_index::Achado<'a> {
    let achados: Vec<_> = mapa
        .buscar(nome)
        .into_iter()
        .filter(|a| a.simbolo.tipo == tipo && !a.simbolo.declaracao)
        .collect();
    assert_eq!(achados.len(), 1, "{nome}: {achados:#?}");
    achados.into_iter().next().unwrap()
}

/// Critério de aceite: os símbolos do programa COBOL e do form Delphi aparecem
/// com arquivo e linhas corretos.
#[test]
fn cobol_e_delphi_da_fixture() {
    let mapa = mapa_de(&fixtures().join("faturamento-exemplo"));
    assert_eq!(mapa.falhas().count(), 0);
    for (arquivo, extrator) in [
        ("cobol/CALCOBR.cbl", "cobol"),
        ("delphi/UFrmCancelamento.pas", "delphi"),
        ("delphi/UFrmCancelamento.dfm", "delphi"),
    ] {
        assert_eq!(mapa.arquivo(arquivo).unwrap().extrator, extrator);
    }

    // COBOL: programa, parágrafos e a variável do valor mínimo.
    let programa = um(&mapa, "CALCOBR", TipoSimbolo::Programa);
    assert_eq!(
        (programa.arquivo, programa.simbolo.linhas),
        ("cobol/CALCOBR.cbl", Linhas::new(7, 92))
    );
    let processar = um(&mapa, "2000-PROCESSAR-FATURA", TipoSimbolo::Paragrafo);
    assert_eq!(processar.simbolo.linhas, Linhas::new(67, 83));
    let gravar = um(&mapa, "3000-GRAVAR-COBRANCA", TipoSimbolo::Paragrafo);
    let quem: Vec<_> = mapa
        .referencias_a(gravar.id)
        .into_iter()
        .map(|u| {
            (
                u.referencia.linha,
                u.referencia.tipo,
                u.origem.map(|o| o.simbolo.nome.clone()),
            )
        })
        .collect();
    assert_eq!(
        quem,
        [(
            81,
            TipoReferencia::Chamada,
            Some("2000-PROCESSAR-FATURA".into())
        )]
    );
    let minimo = um(&mapa, "WS-VALOR-MIN-COBRANCA", TipoSimbolo::Variavel);
    assert_eq!(minimo.simbolo.linhas, Linhas::new(41, 41));
    assert!(
        mapa.referencias_a(minimo.id)
            .iter()
            .any(|u| u.referencia.linha == 78)
    );

    // Delphi: o formulário, o botão e o evento ligado ao método da unit.
    let tela = um(&mapa, "FrmCancelamento", TipoSimbolo::Tela);
    assert_eq!(
        (tela.arquivo, tela.simbolo.linhas),
        ("delphi/UFrmCancelamento.dfm", Linhas::new(1, 68))
    );
    let botoes: Vec<_> = mapa
        .filhos(tela.id)
        .into_iter()
        .filter(|c| {
            c.simbolo
                .detalhe
                .as_deref()
                .unwrap_or("")
                .starts_with("TButton")
        })
        .map(|c| (c.simbolo.nome.clone(), c.simbolo.linhas))
        .collect();
    assert_eq!(
        botoes,
        [
            ("btnCancelar".to_string(), Linhas::new(40, 48)),
            ("btnFechar".to_string(), Linhas::new(49, 57)),
        ]
    );
    let classe = um(&mapa, "TFrmCancelamento", TipoSimbolo::Classe);
    assert_eq!(
        (classe.arquivo, classe.simbolo.linhas),
        ("delphi/UFrmCancelamento.pas", Linhas::new(12, 30))
    );
    let clique = um(&mapa, "btnCancelarClick", TipoSimbolo::Metodo);
    assert_eq!(clique.simbolo.linhas, Linhas::new(80, 104));
    let eventos: Vec<_> = mapa
        .referencias_a(clique.id)
        .into_iter()
        .map(|u| {
            (
                u.arquivo,
                u.referencia.linha,
                u.origem.map(|o| o.simbolo.nome.clone()),
            )
        })
        .collect();
    assert_eq!(
        eventos,
        [(
            "delphi/UFrmCancelamento.dfm",
            47,
            Some("btnCancelar".to_string())
        )]
    );
    // A regra do motivo mínimo está dentro de ValidarCancelamento.
    let validar = um(&mapa, "ValidarCancelamento", TipoSimbolo::Metodo);
    assert!(validar.simbolo.linhas.contem(63) && validar.simbolo.linhas.contem(67));
    let constante = um(&mapa, "TAM_MINIMO_MOTIVO", TipoSimbolo::Constante);
    assert!(
        mapa.referencias_a(constante.id)
            .iter()
            .any(|u| u.referencia.linha == 63 && u.referencia.tipo == TipoReferencia::Uso)
    );
}

/// Critério de aceite: fontes em Windows-1252 são lidos sem erro.
#[test]
fn fontes_em_windows_1252() {
    let tmp = tempfile::tempdir().unwrap();
    let raiz = tmp.path().join("legado");
    fs::create_dir_all(&raiz).unwrap();
    // "COBRANÇA", "Situação", "Cancelar emissão" em Windows-1252.
    fs::write(
        raiz.join("COBR.cbl"),
        b"      * PROGRAMA DE COBRAN\xc7A\n       IDENTIFICATION DIVISION.\n       PROGRAM-ID. COBR.\n       PROCEDURE DIVISION.\n       INICIO.\n           DISPLAY 'Situa\xe7\xe3o'\n           STOP RUN.\n",
    )
    .unwrap();
    fs::write(
        raiz.join("UAviso.pas"),
        b"unit UAviso;\n{ Situa\xe7\xe3o da fatura }\ninterface\nprocedure Avisar;\nimplementation\nprocedure Avisar;\nbegin\n  ShowMessage('Emiss\xe3o cancelada');\nend;\nend.\n",
    )
    .unwrap();
    fs::write(
        raiz.join("frmAviso.frm"),
        b"VERSION 5.00\r\nBegin VB.Form frmAviso\r\n   Caption         =   \"Emiss\xe3o\"\r\n   Begin VB.CommandButton cmdOk\r\n      Caption         =   \"Conclu\xeddo\"\r\n   End\r\nEnd\r\nAttribute VB_Name = \"frmAviso\"\r\nPrivate Sub cmdOk_Click()\r\n   ' A\xe7\xe3o do bot\xe3o\r\n   Unload Me\r\nEnd Sub\r\n",
    )
    .unwrap();
    let mapa = mapa_de(&raiz);
    assert_eq!(mapa.falhas().count(), 0);
    assert_eq!(mapa.arquivos().len(), 3);
    assert_eq!(
        um(&mapa, "INICIO", TipoSimbolo::Paragrafo).simbolo.linhas,
        Linhas::new(5, 7)
    );
    let avisar = um(&mapa, "Avisar", TipoSimbolo::Procedure);
    assert_eq!(
        (avisar.arquivo, avisar.simbolo.linhas),
        ("UAviso.pas", Linhas::new(6, 9))
    );
    let tela = um(&mapa, "frmAviso", TipoSimbolo::Tela);
    assert_eq!(tela.arquivo, "frmAviso.frm");
    let clique = um(&mapa, "cmdOk_Click", TipoSimbolo::Metodo);
    assert_eq!(clique.simbolo.linhas, Linhas::new(9, 12));
    assert_eq!(mapa.referencias_a(clique.id).len(), 1);
}
