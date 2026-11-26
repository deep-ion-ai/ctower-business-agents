import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { Fluxo } from "./analise";
import type { StatusPremissa } from "./registros";

/**
 * Espelho dos comandos da pauta de perguntas pendentes
 * (`ui/src-tauri/src/pauta.rs`, campos em camelCase; #123).
 */
export type Destinatario =
  { tipo: "pessoa"; pessoa: string } | { tipo: "papel"; papel: string };

export type FontePergunta =
  | { tipo: "analise"; id: string; titulo: string; fluxo: Fluxo }
  | { tipo: "ata"; id: string; data: string };

export interface PerguntaDaPauta {
  /** `A-0001/2` ou `R-0001/DV-1`. */
  referencia: string;
  fonte: FontePergunta;
  texto: string;
  /** O que o sistema faz hoje (a afirmação de origem). */
  contexto: string | null;
  destinatario: Destinatario | null;
  tema: string;
  desde: string;
  idadeDias: number;
  /** `HU-0001 · CA-2`. */
  bloqueia: string[];
  /** Documento de origem, relativo à raiz da base. */
  documento: string;
}

export interface ItemDaPauta {
  texto: string;
  destinatario: Destinatario | null;
  perguntas: PerguntaDaPauta[];
  idadeDias: number;
  antiga: boolean;
}

export interface GrupoDaPauta {
  destinatario: Destinatario | null;
  rotulo: string;
  itens: ItemDaPauta[];
}

export interface PessoaDaPauta {
  slug: string;
  nome: string;
  papel: string | null;
}

export interface PremissaDaPauta {
  id: string;
  titulo: string;
  status: StatusPremissa;
}

export interface Pauta {
  titulo: string;
  grupos: GrupoDaPauta[];
  total: number;
  antigas: number;
  pessoas: PessoaDaPauta[];
  papeis: string[];
  premissas: PremissaDaPauta[];
  geradaEm: string;
  /** Texto de e-mail de cada grupo, na ordem dos grupos. */
  emails: string[];
}

export type DestinoDaResposta =
  | { tipo: "so-registrar" }
  | { tipo: "regra"; titulo: string | null; area: string | null }
  | { tipo: "premissa"; premissa: string; status: StatusPremissa };

export interface RespostaRecebida {
  texto: string;
  por: string;
  pessoa: string | null;
  /** `AAAA-MM-DD`. */
  em: string | null;
  fonte: string;
  perguntas: string[];
  destino: DestinoDaResposta;
}

export interface RespostaRegistrada {
  fechadas: string[];
  regra: { id: string; documento: string } | null;
  premissa: string | null;
  avisos: string[];
}

export type FormatoPauta = "markdown" | "word";

export const EXTENSAO_PAUTA: Record<FormatoPauta, string> = {
  markdown: "md",
  word: "docx",
};

/** Operações da pauta. A interface usa {@link apiPautaTauri}; os testes, uma falsa. */
export interface ApiPauta {
  pauta(projeto: string, demanda: string | null): Promise<Pauta>;
  definirDestinatario(
    projeto: string,
    perguntas: string[],
    destinatario: Destinatario | null,
  ): Promise<void>;
  registrarResposta(
    projeto: string,
    resposta: RespostaRecebida,
  ): Promise<RespostaRegistrada>;
  /** Diálogo de salvar; `null` se o analista cancelou. */
  escolherDestino(nome: string, formato: FormatoPauta): Promise<string | null>;
  exportar(
    projeto: string,
    demanda: string | null,
    formato: FormatoPauta,
    destino: string,
  ): Promise<string>;
  copiar(texto: string): Promise<void>;
}

export const apiPautaTauri: ApiPauta = {
  pauta: async (projeto, demanda) =>
    invoke("pauta_perguntas_app", { projeto, demanda }),
  definirDestinatario: async (projeto, perguntas, destinatario) =>
    invoke("definir_destinatario_app", { projeto, perguntas, destinatario }),
  registrarResposta: async (projeto, resposta) =>
    invoke("registrar_resposta_app", { projeto, resposta }),
  escolherDestino: async (nome, formato) =>
    save({
      title: "Exportar a pauta de perguntas",
      defaultPath: `${nome}.${EXTENSAO_PAUTA[formato]}`,
      filters: [
        formato === "markdown"
          ? { name: "Markdown", extensions: ["md"] }
          : { name: "Documento do Word", extensions: ["docx"] },
      ],
    }),
  exportar: async (projeto, demanda, formato, destino) =>
    invoke("exportar_pauta_app", { projeto, demanda, formato, destino }),
  copiar: async (texto) => navigator.clipboard.writeText(texto),
};

/** Valor de um `<select>` de destinatário: `pessoa:<slug>`, `papel:<nome>` ou vazio. */
export function valorDestinatario(d: Destinatario | null): string {
  if (!d) return "";
  return d.tipo === "pessoa" ? `pessoa:${d.pessoa}` : `papel:${d.papel}`;
}

export function destinatarioDoValor(valor: string): Destinatario | null {
  if (valor.startsWith("pessoa:"))
    return { tipo: "pessoa", pessoa: valor.slice(7) };
  if (valor.startsWith("papel:"))
    return { tipo: "papel", papel: valor.slice(6) };
  return null;
}

/**
 * A origem curta de uma pergunta, sem o título da análise (#408): `A-0011/1` vira
 * `A-0011 · pergunta 1`, e `R-0001/DV-2`, `R-0001 · dúvida 2`. Referência fora
 * desses formatos sai como veio.
 */
export function origemCurta(p: Pick<PerguntaDaPauta, "referencia" | "fonte">): string {
  const barra = p.referencia.indexOf("/");
  if (barra < 0) return p.referencia;
  const doc = p.referencia.slice(0, barra);
  const item = p.referencia.slice(barra + 1);
  if (p.fonte.tipo === "analise" && /^\d+$/.test(item))
    return `${doc} · pergunta ${item}`;
  const duvida = /^DV-(\d+)$/.exec(item);
  if (p.fonte.tipo === "ata" && duvida) return `${doc} · dúvida ${duvida[1]}`;
  return p.referencia;
}

export type Agrupamento = "destinatario" | "papel" | "tema";

/** Grupos da pauta no agrupamento escolhido (por destinatário, os do app). */
export function agrupar(
  pauta: Pauta,
  modo: Agrupamento,
): { rotulo: string; itens: ItemDaPauta[] }[] {
  if (modo === "destinatario") return pauta.grupos;
  const grupos = new Map<string, ItemDaPauta[]>();
  for (const item of pauta.grupos.flatMap((g) => g.itens)) {
    let chave: string;
    if (modo === "tema") {
      chave = item.perguntas[0]?.tema ?? "Sem tema";
    } else if (item.destinatario?.tipo === "papel") {
      chave = item.destinatario.papel;
    } else if (item.destinatario?.tipo === "pessoa") {
      const slug = item.destinatario.pessoa;
      chave = pauta.pessoas.find((p) => p.slug === slug)?.papel ?? "Sem papel";
    } else {
      chave = "Sem destinatário";
    }
    grupos.set(chave, [...(grupos.get(chave) ?? []), item]);
  }
  return [...grupos.entries()]
    .sort(([a], [b]) => a.localeCompare(b, "pt-BR"))
    .map(([rotulo, itens]) => ({ rotulo, itens }));
}

/** Data de hoje no fuso local, `AAAA-MM-DD`. */
export function hojeLocal(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}
