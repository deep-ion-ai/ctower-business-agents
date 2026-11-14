//! A base de conhecimento e os projetos dentro dela.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::arquivo;
use crate::caminho::{CaminhoRelativo, e_slug};
use crate::documento::Documento;
use crate::erro::{Erro, Resultado};
use crate::frontmatter::{Frontmatter, FrontmatterProjeto};
use crate::id::{Id, Serie};
use crate::layout;

/// Conteúdo inicial do `regras-do-projeto.md`: só um título e um comentário com
/// as instruções, que não chega ao agente.
pub const PADRAO_REGRAS_DO_PROJETO: &str = include_str!("../modelos/regras-do-projeto.md");

/// Quantas vezes [`Projeto::criar_numerado`] tenta o número seguinte quando outro
/// processo reservou o mesmo antes.
const TENTATIVAS_DE_RESERVA: u32 = 1000;

/// Pasta da base de conhecimento, escolhida pelo analista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base {
    raiz: PathBuf,
}

/// Um projeto dentro da base: a pasta `<base>/<projeto>/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projeto {
    base: PathBuf,
    nome: String,
}

impl Base {
    /// Abre uma base existente.
    pub fn abrir(raiz: impl Into<PathBuf>) -> Resultado<Self> {
        let raiz = raiz.into();
        if !raiz.is_dir() {
            return Err(Erro::BaseInexistente(raiz));
        }
        Ok(Self { raiz })
    }

    /// Cria a base (a pasta e `_templates/`), ou só completa o que faltar se ela já
    /// existir.
    pub fn criar(raiz: impl Into<PathBuf>) -> Resultado<Self> {
        let raiz = raiz.into();
        let templates = raiz.join(layout::TEMPLATES);
        fs::create_dir_all(&templates).map_err(|erro| Erro::PastaDaBase {
            caminho: raiz.clone(),
            erro,
        })?;
        Ok(Self { raiz })
    }

    pub fn raiz(&self) -> &Path {
        &self.raiz
    }

    /// Caminho no disco de um caminho relativo à base.
    pub fn caminho(&self, relativo: &CaminhoRelativo) -> PathBuf {
        relativo.em(&self.raiz)
    }

    /// Pasta dos templates (`_templates/`).
    pub fn templates(&self) -> PathBuf {
        self.raiz.join(layout::TEMPLATES)
    }

    /// Projeto existente na base.
    pub fn projeto(&self, nome: &str) -> Resultado<Projeto> {
        if !e_slug(nome) {
            return Err(Erro::NomeDeProjeto(nome.to_owned()));
        }
        if !self.raiz.join(nome).is_dir() {
            return Err(Erro::ProjetoInexistente(nome.to_owned()));
        }
        Ok(Projeto {
            base: self.raiz.clone(),
            nome: nome.to_owned(),
        })
    }

    /// Cria o projeto descrito em `documento` (pasta com o nome do `id`, as
    /// subpastas e o `projeto.md`). Se o projeto já existir, só cria as pastas que
    /// faltarem e mantém o `projeto.md` que já está lá.
    pub fn criar_projeto(&self, documento: &Documento<FrontmatterProjeto>) -> Resultado<Projeto> {
        let nome = documento.frontmatter.id.clone();
        if !e_slug(&nome) {
            return Err(Erro::NomeDeProjeto(nome));
        }
        let projeto = Projeto {
            base: self.raiz.clone(),
            nome,
        };
        for pasta in layout::PASTAS_DO_PROJETO {
            let relativo = CaminhoRelativo::novo(pasta)?;
            fs::create_dir_all(projeto.caminho(&relativo))
                .map_err(|erro| projeto.erro_io(&relativo, erro))?;
        }
        match projeto.criar(&layout::projeto(), documento) {
            Ok(()) | Err(Erro::JaExiste(_)) => {}
            Err(e) => return Err(e),
        }
        projeto.garantir_regras_do_projeto()?;
        Ok(projeto)
    }

    /// Nomes dos projetos da base (pastas com `projeto.md`), em ordem alfabética.
    pub fn projetos(&self) -> Resultado<Vec<String>> {
        let entradas = fs::read_dir(&self.raiz).map_err(|erro| Erro::PastaDaBase {
            caminho: self.raiz.clone(),
            erro,
        })?;
        let mut nomes = Vec::new();
        for entrada in entradas.flatten() {
            let Ok(nome) = entrada.file_name().into_string() else {
                continue;
            };
            if e_slug(&nome) && entrada.path().join(layout::ARQUIVO_PROJETO).is_file() {
                nomes.push(nome);
            }
        }
        nomes.sort();
        Ok(nomes)
    }

    /// Lê um documento por caminho relativo à base.
    pub fn ler<F: Frontmatter>(&self, relativo: &CaminhoRelativo) -> Resultado<Documento<F>> {
        ler(&self.raiz, relativo, relativo)
    }

    /// Grava um documento (atômico), substituindo o que existir.
    pub fn gravar<F: Frontmatter>(
        &self,
        relativo: &CaminhoRelativo,
        documento: &Documento<F>,
    ) -> Resultado<()> {
        gravar(&self.raiz, relativo, relativo, documento, false)
    }

    /// Lê um arquivo de texto da base (um template, por exemplo).
    pub fn ler_texto(&self, relativo: &CaminhoRelativo) -> Resultado<String> {
        ler_texto(&self.caminho(relativo), relativo)
    }

    /// Grava um arquivo de texto da base (atômico).
    pub fn gravar_texto(&self, relativo: &CaminhoRelativo, texto: &str) -> Resultado<()> {
        arquivo::gravar(&self.caminho(relativo), texto.as_bytes()).map_err(|erro| Erro::Io {
            arquivo: relativo.clone(),
            erro,
        })
    }
}

impl Projeto {
    /// Nome (slug) do projeto, igual ao nome da pasta.
    pub fn nome(&self) -> &str {
        &self.nome
    }

    /// Pasta do projeto na base.
    pub fn raiz(&self) -> PathBuf {
        self.base.join(&self.nome)
    }

    /// Caminho no disco de um caminho relativo à raiz do projeto.
    pub fn caminho(&self, relativo: &CaminhoRelativo) -> PathBuf {
        relativo.em(&self.raiz())
    }

    /// O mesmo caminho, relativo à raiz da base (`<projeto>/<relativo>`).
    pub fn na_base(&self, relativo: &CaminhoRelativo) -> CaminhoRelativo {
        CaminhoRelativo::novo(&self.nome)
            .and_then(|s| s.juntar(relativo.as_str()))
            .expect("slug e caminho relativo formam um caminho válido")
    }

    pub fn existe(&self, relativo: &CaminhoRelativo) -> bool {
        self.caminho(relativo).exists()
    }

    /// Lê o `projeto.md`.
    pub fn ler_projeto(&self) -> Resultado<Documento<FrontmatterProjeto>> {
        let doc: Documento<FrontmatterProjeto> = self.ler(&layout::projeto())?;
        if doc.frontmatter.id != self.nome {
            return Err(Erro::IdDoProjeto {
                id: doc.frontmatter.id,
                pasta: self.nome.clone(),
            });
        }
        Ok(doc)
    }

    /// Cria o `regras-do-projeto.md` com o conteúdo padrão
    /// ([`PADRAO_REGRAS_DO_PROJETO`]) se ele ainda não existir; nunca sobrescreve.
    /// Devolve se criou.
    pub fn garantir_regras_do_projeto(&self) -> Resultado<bool> {
        let relativo = layout::regras_do_projeto();
        match arquivo::criar(
            &self.caminho(&relativo),
            PADRAO_REGRAS_DO_PROJETO.as_bytes(),
        ) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
            Err(erro) => Err(self.erro_io(&relativo, erro)),
        }
    }

    /// Texto do `regras-do-projeto.md`, como está no arquivo (`None` se não
    /// existir).
    pub fn ler_regras_do_projeto(&self) -> Resultado<Option<String>> {
        let relativo = layout::regras_do_projeto();
        match ler_texto(&self.caminho(&relativo), &self.na_base(&relativo)) {
            Ok(texto) => Ok(Some(texto)),
            Err(Erro::Io { erro, .. }) if erro.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Lê um documento por caminho relativo à raiz do projeto.
    pub fn ler<F: Frontmatter>(&self, relativo: &CaminhoRelativo) -> Resultado<Documento<F>> {
        ler(&self.raiz(), relativo, &self.na_base(relativo))
    }

    /// Grava um documento (atômico), substituindo o que existir. Usado a cada
    /// registro numa análise em andamento (gravação contínua, spec seção 7.1).
    pub fn gravar<F: Frontmatter>(
        &self,
        relativo: &CaminhoRelativo,
        documento: &Documento<F>,
    ) -> Resultado<()> {
        gravar(
            &self.raiz(),
            relativo,
            &self.na_base(relativo),
            documento,
            false,
        )
    }

    /// Grava um documento só se ele ainda não existir ([`Erro::JaExiste`] se
    /// existir).
    pub fn criar<F: Frontmatter>(
        &self,
        relativo: &CaminhoRelativo,
        documento: &Documento<F>,
    ) -> Resultado<()> {
        gravar(
            &self.raiz(),
            relativo,
            &self.na_base(relativo),
            documento,
            true,
        )
    }

    /// Próximo identificador livre da série, sem reservar. Para criar o documento,
    /// use [`Projeto::criar_numerado`], que reserva sem colisão.
    pub fn proximo_id(&self, serie: &Serie) -> Resultado<Id> {
        Ok(serie.id(self.maior_numero(serie)? + 1))
    }

    /// Cria um documento com o próximo identificador livre da série, em
    /// `<pasta da série>/<id>.md`, e devolve o identificador e o caminho.
    ///
    /// `montar` recebe o identificador e devolve o documento. Se outro processo
    /// criar o mesmo número antes (o app e o servidor MCP, por exemplo), a criação
    /// falha sem sobrescrever e `montar` é chamada de novo com o número seguinte.
    pub fn criar_numerado<F: Frontmatter>(
        &self,
        serie: &Serie,
        mut montar: impl FnMut(&Id) -> Documento<F>,
    ) -> Resultado<(Id, CaminhoRelativo)> {
        let mut numero = self.maior_numero(serie)? + 1;
        let mut tentativas = 0;
        loop {
            let id = serie.id(numero);
            let relativo = id.arquivo();
            match self.criar(&relativo, &montar(&id)) {
                Ok(()) => return Ok((id, relativo)),
                Err(Erro::JaExiste(_)) if tentativas < TENTATIVAS_DE_RESERVA => {
                    tentativas += 1;
                    numero += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Documentos Markdown do projeto (recursivo), em ordem, relativos à raiz do
    /// projeto. Ignora arquivos e pastas que começam com `.` (temporários da
    /// gravação atômica, `.git`).
    pub fn documentos(&self) -> Resultado<Vec<CaminhoRelativo>> {
        let mut saida = Vec::new();
        let mut pendentes = vec![self.raiz()];
        let raiz = self.raiz();
        let relativo = |caminho: &Path| {
            caminho
                .strip_prefix(&raiz)
                .ok()
                .and_then(|p| CaminhoRelativo::de_caminho(p).ok())
        };
        while let Some(pasta) = pendentes.pop() {
            let entradas = fs::read_dir(&pasta).map_err(|erro| Erro::Io {
                arquivo: match relativo(&pasta) {
                    Some(r) => self.na_base(&r),
                    None => CaminhoRelativo::novo(&self.nome).expect("slug é caminho válido"),
                },
                erro,
            })?;
            for entrada in entradas.flatten() {
                if entrada.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                let caminho = entrada.path();
                let Ok(tipo) = entrada.file_type() else {
                    continue;
                };
                if tipo.is_dir() {
                    pendentes.push(caminho);
                } else if caminho.extension().is_some_and(|e| e == layout::EXTENSAO)
                    && let Some(rel) = relativo(&caminho)
                {
                    saida.push(rel);
                }
            }
        }
        saida.sort();
        Ok(saida)
    }

    fn maior_numero(&self, serie: &Serie) -> Resultado<u32> {
        let relativo = CaminhoRelativo::novo(serie.pasta())?;
        let pasta = self.caminho(&relativo);
        let entradas = match fs::read_dir(&pasta) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(erro) => return Err(self.erro_io(&relativo, erro)),
        };
        let maior = entradas
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter_map(|n| n.strip_suffix(".md").and_then(|stem| serie.numero(stem)))
            .max()
            .unwrap_or(0);
        Ok(maior)
    }

    pub(crate) fn erro_io(&self, relativo: &CaminhoRelativo, erro: io::Error) -> Erro {
        Erro::Io {
            arquivo: self.na_base(relativo),
            erro,
        }
    }
}

fn ler_texto(caminho: &Path, nome: &CaminhoRelativo) -> Resultado<String> {
    let bytes = fs::read(caminho).map_err(|erro| Erro::Io {
        arquivo: nome.clone(),
        erro,
    })?;
    String::from_utf8(bytes).map_err(|_| Erro::NaoUtf8(nome.clone()))
}

fn ler<F: Frontmatter>(
    raiz: &Path,
    relativo: &CaminhoRelativo,
    nome: &CaminhoRelativo,
) -> Resultado<Documento<F>> {
    let texto = ler_texto(&relativo.em(raiz), nome)?;
    Documento::analisar(&texto).map_err(|erro| Erro::Frontmatter {
        arquivo: nome.clone(),
        erro,
    })
}

fn gravar<F: Frontmatter>(
    raiz: &Path,
    relativo: &CaminhoRelativo,
    nome: &CaminhoRelativo,
    documento: &Documento<F>,
    so_criar: bool,
) -> Resultado<()> {
    let texto = documento.para_texto().map_err(|erro| Erro::Frontmatter {
        arquivo: nome.clone(),
        erro,
    })?;
    let destino = relativo.em(raiz);
    let resultado = if so_criar {
        arquivo::criar(&destino, texto.as_bytes())
    } else {
        arquivo::gravar(&destino, texto.as_bytes())
    };
    resultado.map_err(|erro| {
        if erro.kind() == io::ErrorKind::AlreadyExists {
            Erro::JaExiste(nome.clone())
        } else {
            Erro::Io {
                arquivo: nome.clone(),
                erro,
            }
        }
    })
}
