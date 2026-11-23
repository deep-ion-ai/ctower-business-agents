//! Indício de dado pessoal numa coluna do schema, pelo nome e pelo tipo, sem IA
//! (#126).
//!
//! A ferramenta `schema` marca as colunas que parecem guardar dado pessoal ou
//! sensível (LGPD), para o agente da análise de impacto saber onde olhar. É só um
//! indício: o agente confirma no código (o que a coluna guarda, de quem, onde
//! aparece) antes de afirmar, e a falta de indício não prova que a coluna não tem
//! dado pessoal.
//!
//! A heurística é conservadora, para não gerar falso positivo:
//!
//! - o nome é quebrado em palavras (`DT_NASCIMENTO`, `dtNascimento` e `DT-NASC`
//!   dão `DT` e `NASCIMENTO`/`NASC`) e cada regra exige palavras inteiras;
//! - os dados sensíveis do art. 5º, II, da LGPD (saúde, origem racial, religião,
//!   biometria...) valem em qualquer tabela, inclusive como código ou indicador
//!   (`COD_RACA`, `FL_PCD`), e também pelo comentário da coluna;
//! - nomes que identificam pessoa natural por si (CPF, RG, data de nascimento,
//!   nome da mãe, sexo) valem em qualquer tabela;
//! - nomes que só são pessoais quando a tabela é de pessoas (`NOME`, `EMAIL`,
//!   `TELEFONE`, endereço, `DOCUMENTO`, renda, texto livre) exigem que a tabela
//!   ou a própria coluna cite um tipo de pessoa (`CLIENTE`, `FUNCIONARIO`,
//!   `PACIENTE`...): `NOME` em `PRODUTO` não é dado pessoal;
//! - colunas de indicador, quantidade ou situação (`FL_EMAIL_VALIDO`,
//!   `QTD_DEPENDENTES`) e de código ou tipo (`TIPO_DOCUMENTO`, `ID_CLIENTE`) ficam
//!   de fora, salvo os dados sensíveis e os atributos que o código já revela
//!   (`COD_SEXO`);
//! - o texto livre (`OBS`, `OBSERVACAO`...) só conta quando o tipo comporta texto
//!   longo (100 caracteres ou mais, ou CLOB/TEXT).

use serde::Serialize;

/// Indício de dado pessoal numa coluna.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Indicio {
    /// Categoria do dado, em português ("documento de identificação").
    pub categoria: &'static str,
    /// Dado pessoal sensível (LGPD, art. 5º, II).
    pub sensivel: bool,
}

/// Palavras de um nome de coluna ou de tabela, em maiúsculas e sem acento:
/// quebra em `_`, `-`, `.`, espaço e na passagem de minúscula para maiúscula.
fn palavras(nome: &str) -> Vec<String> {
    let mut saida = Vec::new();
    let mut atual = String::new();
    let mut anterior_minuscula = false;
    for c in nome.chars() {
        if !c.is_alphanumeric() {
            if !atual.is_empty() {
                saida.push(std::mem::take(&mut atual));
            }
            anterior_minuscula = false;
            continue;
        }
        if c.is_uppercase() && anterior_minuscula && !atual.is_empty() {
            saida.push(std::mem::take(&mut atual));
        }
        anterior_minuscula = c.is_lowercase();
        atual.extend(sem_acento(c).to_uppercase());
    }
    if !atual.is_empty() {
        saida.push(atual);
    }
    saida
}

fn sem_acento(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'ê' | 'è' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
        'É' | 'Ê' | 'È' | 'Ë' => 'E',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'Ç' => 'C',
        outro => outro,
    }
}

/// Palavras que dizem que a tabela (ou a coluna) é de pessoas.
const PESSOAS: &[&str] = &[
    "CLIENTE",
    "CLIENTES",
    "CLI",
    "PESSOA",
    "PESSOAS",
    "FUNCIONARIO",
    "FUNCIONARIOS",
    "COLABORADOR",
    "EMPREGADO",
    "SERVIDOR",
    "USUARIO",
    "USUARIOS",
    "CONTATO",
    "CONTATOS",
    "PACIENTE",
    "ALUNO",
    "ALUNOS",
    "SEGURADO",
    "BENEFICIARIO",
    "TITULAR",
    "DEPENDENTE",
    "RESPONSAVEL",
    "CANDIDATO",
    "ASSOCIADO",
    "CORRENTISTA",
    "CONSUMIDOR",
    "CONTRIBUINTE",
    "MOTORISTA",
    "PASSAGEIRO",
    "HOSPEDE",
    "MORADOR",
    "SOCIO",
    "PROFISSIONAL",
    "MEDICO",
    "PARTICIPANTE",
    "CIDADAO",
    "ELEITOR",
    "TOMADOR",
    "DEVEDOR",
    "AVALISTA",
    "FIADOR",
];

/// Palavras de coluna que guardam um indicador, quantidade ou situação do dado,
/// e não o dado (`FL_EMAIL_VALIDO`, `QTD_DEPENDENTES`). Não valem para os dados
/// sensíveis: um indicador de deficiência já é dado sensível.
const INDICADORES: &[&str] = &[
    "FL", "FLG", "FLAG", "IND", "QTD", "QT", "QTDE", "STATUS", "SIT", "SITUACAO", "SEQ", "SQ",
    "VALIDO", "VALIDADO", "ACEITA", "ACEITE", "OPCAO",
];

/// Palavras de coluna que guardam um código ou tipo (`TIPO_DOCUMENTO`,
/// `ID_CLIENTE`). Não valem para os dados sensíveis nem para os atributos que o
/// código já revela ([`CODIFICAVEIS`]).
const CODIGOS: &[&str] = &["ID", "TIPO", "TP", "COD", "CD"];

/// Dados sensíveis (LGPD, art. 5º, II), em qualquer tabela.
const SENSIVEIS: &[(&[&str], &str)] = &[
    (&["SAUDE"], "saúde"),
    (&["DOENCA"], "saúde"),
    (&["DIAGNOSTICO"], "saúde"),
    (&["CID10"], "saúde"),
    (&["DEFICIENCIA"], "saúde"),
    (&["PCD"], "saúde"),
    (&["ALERGIA"], "saúde"),
    (&["PRONTUARIO"], "saúde"),
    (&["RACA"], "origem racial ou étnica"),
    (&["ETNIA"], "origem racial ou étnica"),
    (&["RELIGIAO"], "convicção religiosa"),
    (&["CRENCA"], "convicção religiosa"),
    (&["SINDICATO"], "filiação a sindicato"),
    (&["SINDICAL"], "filiação a sindicato"),
    (&["PARTIDO"], "opinião ou filiação política"),
    (&["ORIENTACAO", "SEXUAL"], "vida sexual"),
    (&["BIOMETRIA"], "biometria"),
    (&["BIOMETRICO"], "biometria"),
    (&["BIOMETRICA"], "biometria"),
    (&["IMPRESSAO", "DIGITAL"], "biometria"),
    (&["GENETICO"], "dado genético"),
    (&["GENETICA"], "dado genético"),
];

/// Dados que identificam pessoa natural por si, em qualquer tabela.
const IDENTIFICAM: &[(&[&str], &str)] = &[
    (&["CPF"], "documento de identificação"),
    (&["RG"], "documento de identificação"),
    (&["CNH"], "documento de identificação"),
    (&["PASSAPORTE"], "documento de identificação"),
    (&["NIS"], "documento de identificação"),
    (&["CTPS"], "documento de identificação"),
    (&["TITULO", "ELEITOR"], "documento de identificação"),
    (&["NASCIMENTO"], "data de nascimento"),
    (&["NASC"], "data de nascimento"),
    (&["NOME", "MAE"], "filiação"),
    (&["NOME", "PAI"], "filiação"),
    (&["SEXO"], "sexo ou gênero"),
    (&["GENERO"], "sexo ou gênero"),
    (&["ESTADO", "CIVIL"], "estado civil"),
    (&["NOME", "SOCIAL"], "nome"),
];

/// Categorias de [`IDENTIFICAM`] que um código já revela (`COD_SEXO`).
const CODIFICAVEIS: &[&str] = &["sexo ou gênero", "estado civil"];

/// Dados pessoais quando a tabela (ou a coluna) é de pessoas.
const DE_PESSOAS: &[(&[&str], &str)] = &[
    (&["NOME"], "nome"),
    (&["NM"], "nome"),
    (&["APELIDO"], "nome"),
    (&["DOCUMENTO"], "documento (CPF ou CNPJ)"),
    (&["DOC"], "documento (CPF ou CNPJ)"),
    (&["EMAIL"], "contato"),
    (&["E", "MAIL"], "contato"),
    (&["TELEFONE"], "contato"),
    (&["TEL"], "contato"),
    (&["FONE"], "contato"),
    (&["CELULAR"], "contato"),
    (&["WHATSAPP"], "contato"),
    (&["ENDERECO"], "endereço"),
    (&["LOGRADOURO"], "endereço"),
    (&["CEP"], "endereço"),
    (&["BAIRRO"], "endereço"),
    (&["COMPLEMENTO"], "endereço"),
    (&["SALARIO"], "renda"),
    (&["RENDA"], "renda"),
    (&["FOTO"], "imagem"),
];

/// Texto livre que pode conter dado pessoal ou sensível, quando a tabela é de
/// pessoas e o tipo comporta texto longo.
const TEXTO_LIVRE: &[&str] = &[
    "OBS",
    "OBSERVACAO",
    "OBSERVACOES",
    "ANOTACAO",
    "ANOTACOES",
    "COMENTARIO",
    "HISTORICO",
];

/// Categoria do texto livre.
pub const TEXTO_LIVRE_CATEGORIA: &str = "texto livre (pode conter dado pessoal ou sensível)";

/// Tamanho a partir do qual uma coluna de texto é texto livre.
const TEXTO_LONGO: u32 = 100;

fn contem(palavras: &[String], sequencia: &[&str]) -> bool {
    palavras
        .windows(sequencia.len())
        .any(|janela| janela.iter().zip(sequencia).all(|(p, s)| p == s))
}

fn de_pessoas(palavras: &[String]) -> bool {
    palavras.iter().any(|p| PESSOAS.contains(&p.as_str()))
}

/// Se o tipo da coluna comporta texto longo (`VARCHAR2(400)`, `CLOB`, `TEXT`).
fn texto_longo(tipo: &str) -> bool {
    let t = tipo.to_uppercase();
    if t.contains("CLOB") || t.contains("TEXT") || t.contains("MEMO") {
        return true;
    }
    if !(t.contains("CHAR") || t.contains("STRING")) {
        return false;
    }
    // O tamanho fica entre parênteses: `VARCHAR2(400)`, não o 2 do nome.
    t.split_once('(')
        .map_or("", |(_, resto)| resto)
        .split(|c: char| !c.is_ascii_digit())
        .find(|n| !n.is_empty())
        .and_then(|n| n.parse::<u32>().ok())
        .is_some_and(|n| n >= TEXTO_LONGO)
}

fn achar(lista: &[(&[&str], &'static str)], onde: &[String]) -> Option<&'static str> {
    lista
        .iter()
        .find(|(seq, _)| contem(onde, seq))
        .map(|(_, categoria)| *categoria)
}

fn pessoal(categoria: &'static str) -> Indicio {
    Indicio {
        categoria,
        sensivel: false,
    }
}

/// Indício de dado pessoal na coluna `coluna` da tabela `tabela`, com o tipo
/// (`VARCHAR2(120) NOT NULL`) e o comentário do script, se houver. O comentário só
/// conta para os dados sensíveis e os que identificam a pessoa por si.
pub fn indicio(
    tabela: &str,
    coluna: &str,
    tipo: Option<&str>,
    comentario: Option<&str>,
) -> Option<Indicio> {
    let nome = palavras(coluna);
    let comentario = palavras(comentario.unwrap_or_default());
    let tem = |lista: &[&str]| nome.iter().any(|p| lista.contains(&p.as_str()));

    if let Some(categoria) = achar(SENSIVEIS, &nome).or_else(|| achar(SENSIVEIS, &comentario)) {
        return Some(Indicio {
            categoria,
            sensivel: true,
        });
    }
    if tem(INDICADORES) {
        return None;
    }
    if let Some(categoria) = achar(IDENTIFICAM, &nome)
        && (CODIFICAVEIS.contains(&categoria) || !tem(CODIGOS))
    {
        return Some(pessoal(categoria));
    }
    if tem(CODIGOS) {
        return None;
    }
    if de_pessoas(&palavras(tabela)) || de_pessoas(&nome) {
        if let Some(categoria) = achar(DE_PESSOAS, &nome) {
            return Some(pessoal(categoria));
        }
        if tem(TEXTO_LIVRE) && tipo.is_some_and(texto_longo) {
            return Some(pessoal(TEXTO_LIVRE_CATEGORIA));
        }
    }
    achar(IDENTIFICAM, &comentario).map(pessoal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(tabela: &str, coluna: &str, tipo: &str) -> Option<&'static str> {
        indicio(tabela, coluna, Some(tipo), None).map(|i| i.categoria)
    }

    #[test]
    fn quebra_o_nome_em_palavras() {
        assert_eq!(palavras("DT_NASCIMENTO"), ["DT", "NASCIMENTO"]);
        assert_eq!(palavras("dtNascimento"), ["DT", "NASCIMENTO"]);
        assert_eq!(palavras("CLI-E-MAIL"), ["CLI", "E", "MAIL"]);
        assert_eq!(palavras("app.Endereço"), ["APP", "ENDERECO"]);
    }

    #[test]
    fn colunas_da_tabela_de_clientes() {
        assert_eq!(cat("CLIENTE", "NOME", "VARCHAR2(120)"), Some("nome"));
        assert_eq!(
            cat("CLIENTE", "DOCUMENTO", "VARCHAR2(18)"),
            Some("documento (CPF ou CNPJ)")
        );
        assert_eq!(
            cat("CLIENTE_CONTATO", "EMAIL", "VARCHAR2(120)"),
            Some("contato")
        );
        assert_eq!(
            cat("CLIENTE_CONTATO", "TELEFONE", "VARCHAR2(20)"),
            Some("contato")
        );
        assert_eq!(
            cat("CLIENTE_CONTATO", "LOGRADOURO", "VARCHAR2(150)"),
            Some("endereço")
        );
        assert_eq!(
            cat("CLIENTE_CONTATO", "CEP", "VARCHAR2(9)"),
            Some("endereço")
        );
        assert_eq!(
            cat("CLIENTE_CONTATO", "DT_NASCIMENTO", "DATE"),
            Some("data de nascimento")
        );
        assert_eq!(
            cat("CLIENTE_CONTATO", "OBS_ATENDIMENTO", "VARCHAR2(400)"),
            Some(TEXTO_LIVRE_CATEGORIA)
        );
    }

    #[test]
    fn sem_falso_positivo_nas_colunas_de_negocio() {
        for (tabela, coluna, tipo) in [
            ("CLIENTE", "ID_CLIENTE", "NUMBER(10)"),
            ("CLIENTE", "TIPO_CLIENTE", "VARCHAR2(3)"),
            ("CLIENTE", "ISENTO_ENCARGOS", "CHAR(1)"),
            ("CLIENTE_CONTATO", "ACEITA_AVISO", "CHAR(1)"),
            ("CLIENTE_CONTATO", "DT_ATUALIZACAO", "DATE"),
            ("FATURA", "VL_ORIGINAL", "NUMBER(12,2)"),
            ("FATURA", "MOTIVO_CANCEL", "VARCHAR2(200)"),
            ("PARAMETRO", "DESCRICAO", "VARCHAR2(200)"),
            ("PRODUTO", "NOME", "VARCHAR2(80)"),
            ("FILIAL", "CEP", "VARCHAR2(9)"),
            ("FILIAL", "TELEFONE", "VARCHAR2(20)"),
            ("CLIENTE", "TIPO_DOCUMENTO", "CHAR(1)"),
            ("CLIENTE", "FL_EMAIL_VALIDO", "CHAR(1)"),
            ("CLIENTE", "FL_CPF_VALIDADO", "CHAR(1)"),
            ("CLIENTE", "QTD_DEPENDENTES", "NUMBER(2)"),
            ("CLIENTE", "OBS", "VARCHAR2(20)"),
            ("PRODUTO", "COR", "VARCHAR2(20)"),
            ("NOTA", "ASSINATURA_DIGITAL", "BLOB"),
            ("NOTA_FISCAL", "VL_PIS", "NUMBER(12,2)"),
        ] {
            assert_eq!(cat(tabela, coluna, tipo), None, "{tabela}.{coluna}");
        }
    }

    #[test]
    fn identificacao_e_dado_sensivel_valem_em_qualquer_tabela() {
        assert_eq!(
            cat("PEDIDO", "NR_CPF_COMPRADOR", "VARCHAR2(11)"),
            Some("documento de identificação")
        );
        assert_eq!(
            indicio(
                "ATENDIMENTO",
                "CID10_DIAGNOSTICO",
                Some("VARCHAR2(10)"),
                None
            ),
            Some(Indicio {
                categoria: "saúde",
                sensivel: true
            })
        );
        assert_eq!(cat("FATURA", "NOME_CLIENTE", "VARCHAR2(120)"), Some("nome"));
        // O código ou o indicador de um dado sensível ainda é dado sensível.
        assert_eq!(
            cat("CADASTRO", "COD_RACA", "NUMBER(2)"),
            Some("origem racial ou étnica")
        );
        assert_eq!(cat("CADASTRO", "FL_PCD", "CHAR(1)"), Some("saúde"));
        assert_eq!(
            cat("CADASTRO", "CD_SEXO", "CHAR(1)"),
            Some("sexo ou gênero")
        );
    }

    #[test]
    fn comentario_conta_so_para_identificacao_e_sensivel() {
        let i = indicio(
            "CADASTRO",
            "NR_DOC",
            Some("VARCHAR2(14)"),
            Some("CPF do titular"),
        );
        assert_eq!(i.map(|i| i.categoria), Some("documento de identificação"));
        let i = indicio(
            "ITEM",
            "OBS",
            Some("VARCHAR2(400)"),
            Some("nome do comprador"),
        );
        assert_eq!(i, None);
        let i = indicio(
            "FICHA",
            "OBS",
            Some("VARCHAR2(400)"),
            Some("dados de saúde"),
        );
        assert_eq!(i.map(|i| i.sensivel), Some(true));
    }
}
