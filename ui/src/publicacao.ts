import { invoke } from "@tauri-apps/api/core";

/**
 * Publicação dos documentos aprovados no repositório (#276). Espelha
 * `cba_tools::publicacao` (campos em snake_case).
 */
export type TipoMudanca = "novo" | "alterado" | "removido";

export interface MudancaPrevista {
  /** Relativo à raiz do repositório (`docs/negocio/regras/RN-FAT-001.md`). */
  caminho: string;
  tipo: TipoMudanca;
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
  mudancas: MudancaPrevista[];
  /** A branch da publicação já existe com conteúdo diferente da base (PR/MR pendente). */
  pendente_na_branch: boolean;
  nada_a_publicar: boolean;
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
