//! Passo a passo do manual do usuário (fluxo `manual`, #272).
//!
//! O agente registra os passos (título, instrução em linguagem de usuário, a tela
//! em que o passo acontece e o resultado esperado); o analista inclui, troca ou
//! remove o print de cada passo, com legenda e texto alternativo, ou dispensa o
//! print. O passo sem print e sem dispensa é pendência da aprovação.
//!
//! Os números dos passos são as posições (1, 2, 3…): remover um passo renumera os
//! seguintes. O arquivo do print fica na base (pasta `manual/` da funcionalidade)
//! e o estado guarda só o caminho, relativo à raiz da base do projeto.

use cba_kb::CaminhoRelativo;
use serde::{Deserialize, Serialize};

use crate::analise::{Analise, ErroModelo, Fluxo};

/// Tamanho máximo do texto de um campo do passo.
pub const LIMITE_DO_TEXTO: usize = 4000;
/// Passos no máximo num manual.
pub const MAXIMO_DE_PASSOS: usize = 60;

/// Print de tela de um passo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintDoPasso {
    /// Arquivo PNG na base, relativo à raiz da base do projeto
    /// (`funcionalidades/<area>/<tema>/manual/passo-01-ab12cd34.png`).
    pub arquivo: CaminhoRelativo,
    /// Legenda que aparece abaixo da imagem.
    pub legenda: String,
    /// Texto alternativo (para leitor de tela e quando a imagem não abre).
    pub texto_alternativo: String,
}

/// Passo do manual.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassoManual {
    pub numero: u32,
    pub titulo: String,
    /// O que o usuário faz, em linguagem de usuário final.
    pub instrucao: String,
    /// Tela em que o passo acontece: o marcador "print sugerido: tela X".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tela: Option<String>,
    /// O que o usuário vê depois do passo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resultado: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print: Option<PrintDoPasso>,
    /// O analista dispensou o print deste passo: não é pendência.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub print_dispensado: bool,
}

impl PassoManual {
    /// O passo ainda espera o print (sem print e sem dispensa).
    pub fn sem_print(&self) -> bool {
        self.print.is_none() && !self.print_dispensado
    }
}

/// Passo como o agente registra.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct NovoPasso {
    /// Número do passo a reescrever; sem ele, o passo entra no fim. O número
    /// seguinte ao último também acrescenta.
    #[serde(default)]
    pub numero: Option<u32>,
    pub titulo: String,
    pub instrucao: String,
    #[serde(default)]
    pub tela: Option<String>,
    #[serde(default)]
    pub resultado: Option<String>,
}

/// Texto aparado; vazio vira `None`.
fn opcional(texto: Option<String>) -> Option<String> {
    texto.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

fn exigir(campo: &'static str, valor: &str) -> Result<String, ErroModelo> {
    let v = valor.trim();
    if v.is_empty() {
        return Err(ErroModelo::CampoVazio(campo));
    }
    if v.chars().count() > LIMITE_DO_TEXTO {
        return Err(ErroModelo::PassoInvalido(format!(
            "o campo `{campo}` passa de {LIMITE_DO_TEXTO} caracteres"
        )));
    }
    Ok(v.to_owned())
}

impl Analise {
    fn exigir_manual(&self) -> Result<(), ErroModelo> {
        if self.fluxo == Fluxo::Manual {
            Ok(())
        } else {
            Err(ErroModelo::ForaDoFluxoDoManual(self.id.clone()))
        }
    }

    fn passo_mut(&mut self, numero: u32) -> Result<&mut PassoManual, ErroModelo> {
        self.passos
            .iter_mut()
            .find(|p| p.numero == numero)
            .ok_or(ErroModelo::ItemDesconhecido {
                tipo: "passo",
                numero,
            })
    }

    /// Registra um passo (do agente): novo no fim ou, com `numero` de um passo que
    /// existe, o novo texto dele, com o print e a dispensa preservados. Devolve o
    /// número do passo.
    pub fn registrar_passo(&mut self, novo: NovoPasso) -> Result<u32, ErroModelo> {
        self.exigir_manual()?;
        let titulo = exigir("titulo", &novo.titulo)?;
        let instrucao = exigir("instrucao", &novo.instrucao)?;
        let tela = opcional(novo.tela);
        let resultado = opcional(novo.resultado);
        let proximo = u32::try_from(self.passos.len()).unwrap_or(u32::MAX) + 1;
        match novo.numero {
            Some(n) if n >= 1 && n < proximo => {
                self.aceitar_registro()?;
                let p = self.passo_mut(n)?;
                p.titulo = titulo;
                p.instrucao = instrucao;
                p.tela = tela;
                p.resultado = resultado;
                Ok(n)
            }
            Some(n) if n != proximo => Err(ErroModelo::PassoInvalido(format!(
                "o manual tem {} passo(s); use o número de um deles para reescrever ou {} \
                 (ou omita `numero`) para acrescentar",
                self.passos.len(),
                proximo
            ))),
            _ => {
                if self.passos.len() >= MAXIMO_DE_PASSOS {
                    return Err(ErroModelo::PassoInvalido(format!(
                        "o manual já tem {MAXIMO_DE_PASSOS} passos; junte passos parecidos"
                    )));
                }
                self.aceitar_registro()?;
                self.passos.push(PassoManual {
                    numero: proximo,
                    titulo,
                    instrucao,
                    tela,
                    resultado,
                    print: None,
                    print_dispensado: false,
                });
                Ok(proximo)
            }
        }
    }

    /// Remove o passo e renumera os seguintes. Devolve o print que ele tinha.
    pub fn remover_passo(&mut self, numero: u32) -> Result<Option<PrintDoPasso>, ErroModelo> {
        self.exigir_manual()?;
        self.recusar_se_aprovada()?;
        let i = self.passos.iter().position(|p| p.numero == numero).ok_or(
            ErroModelo::ItemDesconhecido {
                tipo: "passo",
                numero,
            },
        )?;
        let removido = self.passos.remove(i);
        for (n, p) in self.passos.iter_mut().enumerate() {
            p.numero = u32::try_from(n).unwrap_or(u32::MAX) + 1;
        }
        Ok(removido.print)
    }

    /// Novo título, instrução, tela e resultado do passo (do analista), com o
    /// print preservado.
    pub fn editar_passo(&mut self, numero: u32, novo: NovoPasso) -> Result<(), ErroModelo> {
        self.exigir_manual()?;
        self.recusar_se_aprovada()?;
        let titulo = exigir("titulo", &novo.titulo)?;
        let instrucao = exigir("instrucao", &novo.instrucao)?;
        let p = self.passo_mut(numero)?;
        p.titulo = titulo;
        p.instrucao = instrucao;
        p.tela = opcional(novo.tela);
        p.resultado = opcional(novo.resultado);
        Ok(())
    }

    /// Põe, troca ou (com `None`) tira o print do passo. Devolve o anterior.
    pub fn definir_print(
        &mut self,
        numero: u32,
        print: Option<PrintDoPasso>,
    ) -> Result<Option<PrintDoPasso>, ErroModelo> {
        self.exigir_manual()?;
        self.recusar_se_aprovada()?;
        let print = match print {
            Some(p) => Some(PrintDoPasso {
                legenda: exigir("legenda", &p.legenda)?,
                texto_alternativo: exigir("texto_alternativo", &p.texto_alternativo)?,
                arquivo: p.arquivo,
            }),
            None => None,
        };
        let passo = self.passo_mut(numero)?;
        if print.is_some() {
            passo.print_dispensado = false;
        }
        Ok(std::mem::replace(&mut passo.print, print))
    }

    /// Nova legenda e novo texto alternativo do print do passo.
    pub fn editar_legenda(
        &mut self,
        numero: u32,
        legenda: &str,
        texto_alternativo: &str,
    ) -> Result<(), ErroModelo> {
        self.exigir_manual()?;
        self.recusar_se_aprovada()?;
        let legenda = exigir("legenda", legenda)?;
        let alt = exigir("texto_alternativo", texto_alternativo)?;
        let passo = self.passo_mut(numero)?;
        let print = passo
            .print
            .as_mut()
            .ok_or_else(|| ErroModelo::PassoInvalido(format!("o passo {numero} não tem print")))?;
        print.legenda = legenda;
        print.texto_alternativo = alt;
        Ok(())
    }

    /// Dispensa (ou volta a pedir) o print do passo.
    pub fn dispensar_print(&mut self, numero: u32, dispensado: bool) -> Result<(), ErroModelo> {
        self.exigir_manual()?;
        self.recusar_se_aprovada()?;
        self.passo_mut(numero)?.print_dispensado = dispensado;
        Ok(())
    }

    /// Arquivos de print em uso nos passos.
    pub fn prints(&self) -> impl Iterator<Item = &CaminhoRelativo> {
        self.passos
            .iter()
            .filter_map(|p| p.print.as_ref().map(|x| &x.arquivo))
    }

    fn recusar_se_aprovada(&self) -> Result<(), ErroModelo> {
        if self.estado == crate::analise::EstadoAnalise::Aprovada {
            Err(ErroModelo::AnaliseAprovada(self.id.clone()))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analise::EstadoAnalise;
    use cba_kb::Id;

    fn manual() -> Analise {
        Analise::nova(
            Id::analisar("A-0001").unwrap(),
            "p",
            Fluxo::Manual,
            "F-CAN-001 · Cancelar fatura · manual",
            "",
            1,
        )
    }

    fn passo(titulo: &str) -> NovoPasso {
        NovoPasso {
            titulo: titulo.into(),
            instrucao: format!("Faça {titulo}."),
            tela: Some("Cancelamento".into()),
            ..Default::default()
        }
    }

    fn print(nome: &str) -> PrintDoPasso {
        PrintDoPasso {
            arquivo: CaminhoRelativo::novo(&format!("funcionalidades/c/c/manual/{nome}.png"))
                .unwrap(),
            legenda: "Tela".into(),
            texto_alternativo: "Tela de cancelamento".into(),
        }
    }

    #[test]
    fn registra_reescreve_e_recusa_numero_fora_da_sequencia() {
        let mut a = manual();
        assert_eq!(a.registrar_passo(passo("abrir")).unwrap(), 1);
        assert_eq!(a.registrar_passo(passo("buscar")).unwrap(), 2);
        assert_eq!(a.estado, EstadoAnalise::EmAndamento);
        a.definir_print(1, Some(print("p1"))).unwrap();
        let n = a
            .registrar_passo(NovoPasso {
                numero: Some(1),
                ..passo("abrir a tela")
            })
            .unwrap();
        assert_eq!(n, 1);
        assert_eq!(a.passos[0].titulo, "abrir a tela");
        assert!(a.passos[0].print.is_some(), "o print fica");
        assert_eq!(
            a.registrar_passo(NovoPasso {
                numero: Some(3),
                ..passo("confirmar")
            })
            .unwrap(),
            3
        );
        assert!(matches!(
            a.registrar_passo(NovoPasso {
                numero: Some(9),
                ..passo("x")
            }),
            Err(ErroModelo::PassoInvalido(_))
        ));
    }

    #[test]
    fn remover_renumera_e_devolve_o_print() {
        let mut a = manual();
        for t in ["a", "b", "c"] {
            a.registrar_passo(passo(t)).unwrap();
        }
        a.definir_print(2, Some(print("p2"))).unwrap();
        let antigo = a.remover_passo(2).unwrap();
        assert_eq!(
            antigo.unwrap().arquivo.as_str(),
            "funcionalidades/c/c/manual/p2.png"
        );
        let numeros: Vec<(u32, &str)> = a
            .passos
            .iter()
            .map(|p| (p.numero, p.titulo.as_str()))
            .collect();
        assert_eq!(numeros, [(1, "a"), (2, "c")]);
    }

    #[test]
    fn print_e_dispensa() {
        let mut a = manual();
        a.registrar_passo(passo("a")).unwrap();
        assert!(a.passos[0].sem_print());
        a.dispensar_print(1, true).unwrap();
        assert!(!a.passos[0].sem_print());
        a.definir_print(1, Some(print("p"))).unwrap();
        assert!(!a.passos[0].print_dispensado, "pôr o print tira a dispensa");
        a.editar_legenda(1, "Nova", "Alt novo").unwrap();
        assert_eq!(a.passos[0].print.as_ref().unwrap().legenda, "Nova");
        let antigo = a.definir_print(1, None).unwrap();
        assert!(antigo.is_some());
        assert!(a.passos[0].sem_print());
        assert!(matches!(
            a.editar_legenda(1, "x", "y"),
            Err(ErroModelo::PassoInvalido(_))
        ));
        assert!(matches!(
            a.definir_print(
                1,
                Some(PrintDoPasso {
                    legenda: " ".into(),
                    ..print("p")
                })
            ),
            Err(ErroModelo::CampoVazio("legenda"))
        ));
    }

    #[test]
    fn so_no_fluxo_manual_e_nao_depois_de_aprovada() {
        let mut a = manual();
        a.fluxo = Fluxo::Explicar;
        assert!(matches!(
            a.registrar_passo(passo("a")),
            Err(ErroModelo::ForaDoFluxoDoManual(_))
        ));
        let mut a = manual();
        a.registrar_passo(passo("a")).unwrap();
        a.estado = EstadoAnalise::Aprovada;
        assert!(matches!(
            a.dispensar_print(1, true),
            Err(ErroModelo::AnaliseAprovada(_))
        ));
    }
}
