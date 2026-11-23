import { invoke } from "@tauri-apps/api/core";

/**
 * Publicação da documentação aprovada no repositório (#276, #289). Espelha
 * `cba_tools::publicacao` (campos em snake_case).
 */
export type TipoMudanca = "novo" | "alterado" | "removido";

export interface MudancaPrevista {
  /** Relativo à raiz do repositório (`docs/negocio/regras/RN-FAT-001.md`). */
  caminho: string;
  tipo: TipoMudanca;
}

/** Um documento que muda (espelha `cba_tools::publicacao::MudancaDeDocumento`). */
export interface MudancaDeDocumento {
  /** `tipo` do frontmatter (`regra`, `termo`, `pergunta`...). */
  tipo: string;
  /** `RN-COB-001`, `multa`, `A-0001`. */
  id: string;
  titulo: string;
  mudanca: TipoMudanca;
  /** Página onde o documento fica (ou ficava), relativa à raiz do repositório. */
  pagina: string;
}

/** O que muda por documento (#289). */
export interface ResumoDosDocumentos {
  documentos: MudancaDeDocumento[];
  /** Documentos da publicação no formato antigo que só mudam de formato. */
  reformatados: number;
  /** "2 regras novas, 1 alterada; 1 termo novo"; vazio sem mudança. */
  texto: string;
}

/** O que muda na branch base do destino se a documentação for publicada agora. */
export interface PreviaPublicacao {
  /** `docs/negocio em GitHub dono/repo`. */
  destino: string;
  repositorio: string;
  /** Repositório próprio da documentação (e não uma pasta do código). */
  independente: boolean;
  base: string;
  /** Branch da publicação (`docs/cba-<projeto>`). */
  branch: string;
  /** Documentos aprovados na base. */
  aprovados: number;
  /** Páginas geradas. */
  paginas: number;
  /** O que muda por documento. */
  documentos: ResumoDosDocumentos;
  /** Páginas afetadas: novas, alteradas e removidas na branch base. */
  mudancas: MudancaPrevista[];
  /** A base tem a publicação no formato antigo (os `.md` da base), que é substituída. */
  formato_antigo: boolean;
  /** A branch da publicação já existe com conteúdo diferente da base (PR/MR pendente). */
  pendente_na_branch: boolean;
  /** O PR/MR pendente já tem esta versão: nada a enviar. */
  branch_atualizada: boolean;
  nada_a_publicar: boolean;
  /** Documentos aprovados que ficaram fora das páginas. */
  avisos: string[];
}

/** Espelha `cba_repos::escrita::Publicado`. */
export interface Publicado {
  situacao: "aberto" | "atualizado" | "sem_mudancas";
  branch: string;
  base: string;
  commit: string | null;
  /** Link do PR/MR. */
  link: string | null;
}

export interface Publicacao {
  previa: PreviaPublicacao;
  publicado: Publicado;
}

export interface ApiPublicacao {
  previa(projeto: string): Promise<PreviaPublicacao>;
  publicar(projeto: string): Promise<Publicacao>;
}

export const apiPublicacaoTauri: ApiPublicacao = {
  previa: (projeto) => invoke<PreviaPublicacao>("previa_publicacao_docs_app", { projeto }),
  publicar: (projeto) => invoke<Publicacao>("publicar_docs_app", { projeto }),
};
