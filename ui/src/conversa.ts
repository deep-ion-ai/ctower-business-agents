import { invoke } from "@tauri-apps/api/core";
import type { Plataforma } from "./clientes";

/**
 * Espelho dos comandos dos clientes da conversa (`ui/src-tauri/src/conversa.rs` e
 * `cba_session::passagem`): Claude Desktop, terminal do analista e a passagem entre
 * eles (#28).
 */
export type ClienteConversa = "terminalEmbutido" | "claudeDesktop" | "terminalDoAnalista";

export interface OpcaoCliente {
  cliente: ClienteConversa;
  disponivel: boolean;
  /** Por que não está disponível. */
  motivo?: string;
}

export interface OpcoesConversa {
  plataforma: Plataforma;
  preferido: ClienteConversa | null;
  /** Os três clientes, o preferido (se disponível) primeiro. */
  opcoes: OpcaoCliente[];
  /** Sessão mais recente da análise, para retomar pelo identificador. */
  sessao: string | null;
  /** Pedido de uma conversa nova (iniciar ou retomar a análise). */
  pedido: string;
  linkDesktop: string | null;
  linkTerminal: string | null;
  /** `cd <pasta> && claude '<pedido>'`. */
  comandoTerminal: string | null;
  /** `cd <pasta> && claude --resume <id>`. */
  comandoRetomada: string | null;
  problema?: string;
}

/** Como o `claude --desktop --resume` terminou. */
export interface ResultadoDesktop {
  aberto: boolean;
  codigo: number | null;
  mensagem: string;
}

/** Operações dos clientes da conversa. A interface usa {@link apiConversaTauri}; os testes, uma falsa. */
export interface ApiConversa {
  opcoes(sistema: string, analise: string): Promise<OpcoesConversa>;
  /** Abre o link de uma conversa nova no Desktop ou no terminal do analista. */
  abrirPorLink(
    sistema: string,
    analise: string,
    cliente: "claudeDesktop" | "terminalDoAnalista",
  ): Promise<void>;
  /** `claude --desktop --resume <id>` da sessão da análise. */
  retomarNoDesktop(sistema: string, analise: string): Promise<ResultadoDesktop>;
  /** Avisa que a sessão passou ao Desktop pelo `/desktop`. */
  passouAoDesktop(sistema: string, analise: string): Promise<void>;
  /** Copia um texto para a área de transferência. */
  copiar(texto: string): Promise<void>;
}

export const apiConversaTauri: ApiConversa = {
  opcoes: (sistema, analise) => invoke("opcoes_conversa_analise", { sistema, analise }),
  abrirPorLink: (sistema, analise, cliente) =>
    invoke("abrir_conversa_por_link", { sistema, analise, cliente }),
  retomarNoDesktop: (sistema, analise) =>
    invoke("retomar_conversa_no_desktop", { sistema, analise }),
  passouAoDesktop: (sistema, analise) =>
    invoke("conversa_passou_ao_desktop", { sistema, analise }),
  async copiar(texto) {
    if (!navigator.clipboard) throw new Error("área de transferência indisponível");
    await navigator.clipboard.writeText(texto);
  },
};

/** Opções válidas vindas do Rust, ou `null` (comando ausente ou resposta inesperada). */
export function comoOpcoes(valor: unknown): OpcoesConversa | null {
  if (typeof valor !== "object" || valor === null) return null;
  const o = valor as Partial<OpcoesConversa>;
  return Array.isArray(o.opcoes) && typeof o.pedido === "string" ? (o as OpcoesConversa) : null;
}

/** O primeiro cliente disponível (o preferido, quando disponível). */
export function principal(opcoes: OpcoesConversa | null): ClienteConversa {
  return opcoes?.opcoes.find((o) => o.disponivel)?.cliente ?? "terminalEmbutido";
}

export function desktopDisponivel(opcoes: OpcoesConversa | null): boolean {
  return opcoes?.opcoes.some((o) => o.cliente === "claudeDesktop" && o.disponivel) ?? false;
}
