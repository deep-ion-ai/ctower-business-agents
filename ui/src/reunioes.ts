import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { AnaliseCriada, EstadoAnalise } from "./analise";
import type { StatusAta, TranscriptGuardado } from "./demandas";

/**
 * Espelho dos comandos da reunião (`ui/src-tauri/src/reunioes.rs`, campos em
 * camelCase): a prévia do transcript, a nova reunião, a ata ao vivo e a confirmação
 * dos participantes (#141), a aprovação da ata com o descarte do transcript e os
 * próximos passos que abrem análises (#142).
 */
export type FormatoTranscript = "vtt" | "docx" | "texto_teams" | "desconhecido";

export const ROTULO_FORMATO: Record<FormatoTranscript, string> = {
  vtt: "WebVTT do Teams (.vtt)",
  docx: "Word exportado do Teams (.docx)",
  texto_teams: "Texto copiado do Teams",
  desconhecido: "Formato não reconhecido (texto corrido)",
};

/** O texto colado ou o arquivo anexado. */
export type EntradaTranscript =
  | { tipo: "texto"; texto: string }
  | { tipo: "arquivo"; caminho: string };

export interface PreviaTranscript {
  formato: FormatoTranscript;
  falas: number;
  falantes: { nome: string; falas: number }[];
  /** Falas sem falante identificado. */
  semFalante: number;
  avisos: { codigo: string; mensagem: string }[];
  /** Há ao menos uma fala. */
  podeCriar: boolean;
}

/** Demanda da reunião: uma aberta do projeto ou uma nova. */
export type DemandaDaReuniao =
  | { tipo: "existente"; id: string }
  | { tipo: "nova"; titulo: string; chave?: string | null };

/** Reunião criada, com a análise do fluxo `reuniao` e o pedido que abre a conversa. */
export interface ReuniaoCriada extends AnaliseCriada {
  /** `R-0001`. */
  reuniao: string;
  /** `D-0001`. */
  demanda: string;
}

export type TipoItemAta =
  | "decisao"
  | "necessidade"
  | "regra"
  | "pendencia"
  | "duvida"
  | "proximo_passo";

export const TITULO_TIPO_ITEM: Record<TipoItemAta, string> = {
  decisao: "Decisões",
  necessidade: "Necessidades",
  regra: "Regras citadas",
  pendencia: "Pendências",
  duvida: "Dúvidas em aberto",
  proximo_passo: "Próximos passos",
};

export const TIPOS_ITEM: TipoItemAta[] = [
  "decisao",
  "necessidade",
  "regra",
  "pendencia",
  "duvida",
  "proximo_passo",
];

export interface CitacaoVista {
  falante: string;
  /** `HH:MM:SS`. */
  inicio: string;
  trecho: string;
  /** "conferida", "conferida em AAAA-MM-DD" ou "não confere". */
  status: string;
}

export interface ItemAtaVista {
  /** `D-1`, `P-2`. */
  id: string;
  tipo: TipoItemAta;
  texto: string;
  responsavel: string | null;
  prazo: string | null;
  referencias: string[];
  citacoes: CitacaoVista[];
  /** `arquivo:linhas`. */
  evidencias: string[];
}

export interface PessoaVista {
  slug: string;
  nome: string;
}

export interface ParticipanteVista {
  nome: string;
  /** Papel confirmado pelo analista. */
  papel: string | null;
  /** Pessoa do catálogo (slug). */
  pessoa: string | null;
  /** Sugestão do catálogo, sem IA. */
  catalogo: { pessoa: PessoaVista; papel: string | null; reuniao: string | null } | null;
  /** Pessoas com o mesmo nome no catálogo: o analista escolhe. */
  candidatos: PessoaVista[];
  /** Papel proposto pelo agente (`propor_papel`). */
  proposta: { papel: string; justificativa: string | null; citacoes: CitacaoVista[] } | null;
}

export interface AtaVista {
  id: string;
  demanda: string;
  data: string;
  status: StatusAta;
  objetivo: string | null;
  resumo: string | null;
  participantes: ParticipanteVista[];
  itens: ItemAtaVista[];
  /** A ata, relativa à raiz da base. */
  documento: string | null;
  /** Papéis padrão da base. */
  papeis: { nome: string; descricao: string | null }[];
  transcript: TranscriptGuardado | null;
}

/** Participante confirmado na tela. */
export interface ConfirmacaoParticipante {
  nome: string;
  papel: string | null;
  /** Pessoa do catálogo escolhida, quando o nome casa com mais de uma. */
  pessoa: string | null;
  /** Outros nomes da reunião que são a mesma pessoa. */
  mesmos: string[];
}

/** Se a ata pode ser aprovada (`cba_tools::aprovacao_ata::ProntidaoAta`). */
export interface ProntidaoAta {
  reuniao: string;
  status: StatusAta;
  estadoAnalise: EstadoAnalise;
  /** Ata aprovada, transcript apagado e análise aprovada. */
  concluida: boolean;
  podeAprovar: boolean;
  pendencias: string[];
  avisos: string[];
  transcriptGuardado: boolean;
  /** Citações que viram "conferida em <data>" na aprovação. */
  citacoesConferiveis: number;
  itens: number;
  /** O segundo aviso: o transcript é apagado e as citações deixam de ser conferíveis. */
  avisoDescarte: string;
}

/** Resultado da aprovação da ata. */
export interface AtaAprovada {
  reuniao: string;
  jaAprovada: boolean;
  transcriptApagado: boolean;
  citacoesMarcadas: number;
  /** `AAAA-MM-DD`. */
  data: string;
  avisos: string[];
}

/** O que fazer com um próximo passo da ata aprovada. */
export type AcaoProximoPasso =
  | { tipo: "impacto"; necessidade: string | null }
  | { tipo: "perguntar"; pergunta: string };

/** Operações das telas da reunião. A interface usa {@link apiReuniaoTauri}; os testes, uma falsa. */
export interface ApiReuniao {
  /** Diálogo para escolher o `.vtt` ou o `.docx`; `null` se cancelado. */
  escolherArquivo(): Promise<string | null>;
  previaTranscript(entrada: EntradaTranscript): Promise<PreviaTranscript>;
  /** Recusado (no app também) sem o aviso de privacidade aceito. */
  criarReuniao(
    projeto: string,
    demanda: DemandaDaReuniao,
    entrada: EntradaTranscript,
    avisoAceito: boolean,
  ): Promise<ReuniaoCriada>;
  ataReuniao(projeto: string, reuniao: string): Promise<AtaVista>;
  confirmarParticipantes(
    projeto: string,
    reuniao: string,
    confirmacoes: ConfirmacaoParticipante[],
  ): Promise<AtaVista>;
  prontidaoAta(projeto: string, analise: string): Promise<ProntidaoAta>;
  /** Recusado (no app também) sem a confirmação do segundo aviso. */
  aprovarAta(projeto: string, analise: string, confirmacao: boolean): Promise<AtaAprovada>;
  /** Abre a análise de um próximo passo (`PP-1`), ligada à demanda da reunião. */
  analisarProximoPasso(
    projeto: string,
    reuniao: string,
    item: string,
    acao: AcaoProximoPasso,
  ): Promise<AnaliseCriada>;
}

export const apiReuniaoTauri: ApiReuniao = {
  async escolherArquivo() {
    const escolhido = await open({
      multiple: false,
      directory: false,
      title: "Transcript da reunião",
      filters: [{ name: "Transcript do Teams", extensions: ["vtt", "docx", "txt"] }],
    });
    return typeof escolhido === "string" ? escolhido : null;
  },
  previaTranscript: (entrada) => invoke("previa_transcript_app", { entrada }),
  criarReuniao: (projeto, demanda, entrada, avisoAceito) =>
    invoke("criar_reuniao_app", { projeto, demanda, entrada, avisoAceito }),
  ataReuniao: (projeto, reuniao) => invoke("ata_reuniao_app", { projeto, reuniao }),
  confirmarParticipantes: (projeto, reuniao, confirmacoes) =>
    invoke("confirmar_participantes_app", { projeto, reuniao, confirmacoes }),
  prontidaoAta: (projeto, analise) => invoke("prontidao_ata_app", { projeto, analise }),
  aprovarAta: (projeto, analise, confirmacao) =>
    invoke("aprovar_ata_app", { projeto, analise, confirmacao }),
  analisarProximoPasso: (projeto, reuniao, item, acao) =>
    invoke("analisar_proximo_passo_app", { projeto, reuniao, item, acao }),
};

/** Nome do arquivo de um caminho (Windows ou Unix). */
export function nomeDoArquivo(caminho: string): string {
  return caminho.split(/[\\/]/).pop() ?? caminho;
}
