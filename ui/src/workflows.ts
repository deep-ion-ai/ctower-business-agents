import { invoke } from "@tauri-apps/api/core";

/**
 * Espelho dos comandos de Configurações › Workflows (`ui/src-tauri/src/workflows.rs`,
 * campos em camelCase; #372).
 */

/** De onde vem a versão de um workflow (`OrigemWorkflow`). */
export type OrigemWorkflow = "padrao" | "global" | "projeto";

/** Onde fica a cópia editada: na base (`projeto`) ou para todos os projetos (`global`). */
export type Destino = "projeto" | "global";

type Situacao = "ok" | "ausente" | "desatualizado" | "conflito" | "divergente";

/** Uma cópia de um workflow (`Copia`). */
export type Copia = {
  origem: OrigemWorkflow;
  arquivo: string;
  hashPadrao: string | null;
  /** O padrão do app mudou desde a cópia (ou não se sabe de qual ela partiu). */
  padraoMudou: boolean;
} & (
  | { situacao: "ausente" }
  | { situacao: "valida" }
  | { situacao: "invalida"; erros: string }
  | { situacao: "ilegivel"; erro: string }
);

export interface Fase {
  titulo: string;
  detalhe: string | null;
}

/** Erro ou aviso da validação; `linha` conta a partir de 1. */
export interface Problema {
  linha: number | null;
  mensagem: string;
  /** Erro (impede salvar) ou só aviso. */
  erro: boolean;
}

export interface Validacao {
  problemas: Problema[];
  fases: Fase[];
}

export interface ArquivoInstalado {
  arquivo: string;
  situacao: Situacao;
  /** A origem gravada no cabeçalho do arquivo instalado. */
  origem: OrigemWorkflow | null;
}

export interface InstalacaoProjeto {
  projeto: string;
  nome: string;
  arquivos: ArquivoInstalado[];
  erro: string | null;
}

export interface WorkflowVista {
  tipo: string;
  /** `cba-<tipo>`. */
  nome: string;
  rotulo: string;
  origemEfetiva: OrigemWorkflow;
  fases: Fase[];
  /** A cópia na base; `null` sem base escolhida. */
  projeto: Copia | null;
  global: Copia;
  instalacoes: InstalacaoProjeto[];
}

export interface LeituraWorkflow {
  tipo: string;
  nome: string;
  rotulo: string;
  /** `null`: o padrão do app, só leitura. */
  destino: Destino | null;
  arquivo: string | null;
  /** O script da cópia, sem as linhas do app (ou o padrão). */
  texto: string;
  padrao: string;
  existe: boolean;
  padraoMudou: boolean;
  /** Marca da versão do arquivo; vai de volta em `salvar`. */
  versao: string;
  validacao: Validacao;
}

export interface LinhaDiferenca {
  tipo: "igual" | "removida" | "acrescentada";
  texto: string;
}

export interface MudancaWorkflow {
  /** `false` quando a validação recusou o texto (nada foi gravado). */
  salvo: boolean;
  leitura: LeituraWorkflow;
  /** Arquivos instalados nos projetos regravados com a versão nova. */
  reinstalados: number;
  avisos: string[];
}

export interface ApiWorkflows {
  workflows(): Promise<WorkflowVista[]>;
  /** `destino` `null`: o padrão do app. */
  ler(tipo: string, destino: Destino | null): Promise<LeituraWorkflow>;
  validar(tipo: string, texto: string): Promise<Validacao>;
  /** Diferença do padrão atual para `texto`. */
  diferenca(tipo: string, texto: string): Promise<LinhaDiferenca[]>;
  duplicar(tipo: string, destino: Destino): Promise<MudancaWorkflow>;
  /** Recusa com erro iniciado por `conflito:` se o arquivo mudou desde a leitura. */
  salvar(
    tipo: string,
    destino: Destino,
    texto: string,
    versaoLida: string,
    padraoRevisto: boolean,
  ): Promise<MudancaWorkflow>;
  restaurar(tipo: string, destino: Destino): Promise<MudancaWorkflow>;
  /** Leva para uma cópia a edição feita à mão no arquivo instalado de um projeto. */
  levarEdicao(
    projeto: string,
    tipo: string,
    arquivo: string,
    destino: Destino,
  ): Promise<MudancaWorkflow>;
}

export const apiWorkflowsTauri: ApiWorkflows = {
  workflows: () => invoke("workflows_app"),
  ler: (tipo, destino) => invoke("ler_workflow_app", { tipo, destino }),
  validar: (tipo, texto) => invoke("validar_workflow_app", { tipo, texto }),
  diferenca: (tipo, texto) => invoke("diferenca_workflow_app", { tipo, texto }),
  duplicar: (tipo, destino) => invoke("duplicar_workflow_app", { tipo, destino }),
  salvar: (tipo, destino, texto, versaoLida, padraoRevisto) =>
    invoke("salvar_workflow_app", { tipo, destino, texto, versaoLida, padraoRevisto }),
  restaurar: (tipo, destino) => invoke("restaurar_workflow_app", { tipo, destino }),
  levarEdicao: (projeto, tipo, arquivo, destino) =>
    invoke("levar_edicao_workflow_app", { projeto, tipo, arquivo, destino }),
};

/** Como a tela chama cada origem. */
export const ORIGEM: Record<OrigemWorkflow, string> = {
  padrao: "padrão do app",
  global: "cópia para todos os projetos",
  projeto: "cópia na base",
};

/** Como a tela chama cada destino de cópia. */
export const DESTINO: Record<Destino, string> = {
  projeto: "Cópia na base",
  global: "Cópia para todos os projetos",
};

/** Resumo do que mudou nos projetos depois de salvar, duplicar ou restaurar. */
export function recadoDaMudanca(m: MudancaWorkflow, feito: string): string {
  const reinstalados =
    m.reinstalados === 0
      ? ""
      : m.reinstalados === 1
        ? " O workflow instalado num projeto foi atualizado."
        : ` Os workflows instalados foram atualizados em ${m.reinstalados} lugares.`;
  return `${feito}${reinstalados}`;
}
