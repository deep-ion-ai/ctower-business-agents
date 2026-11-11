import { invoke } from "@tauri-apps/api/core";

/** Espelho de `cba_session::clientes::EstadoClientes` (JSON do comando `estado_clientes`). */
export type Plataforma = "windows" | "macOs" | "linux";

export interface Passo {
  texto: string;
  comando?: string;
  link?: string;
}

export type InstalacaoClaudeCode =
  | { estado: "naoEncontrado" }
  | {
      estado: "encontrado";
      caminho: string;
      origem: "path" | "localPadrao";
      versao: string | null;
    };

export type LoginClaudeCode =
  | { estado: "naoVerificado" }
  | { estado: "logado"; metodo: string | null; provedor: string | null }
  | { estado: "naoLogado" }
  | { estado: "indeterminado"; motivo: string };

export type InstalacaoClaudeDesktop =
  | { estado: "naoSuportado" }
  | { estado: "naoEncontrado" }
  | { estado: "encontrado"; caminho: string };

export interface EstadoClientes {
  plataforma: Plataforma;
  claudeCode: {
    instalacao: InstalacaoClaudeCode;
    login: LoginClaudeCode;
    orientacao: Passo[];
  };
  claudeDesktop: {
    instalacao: InstalacaoClaudeDesktop;
    orientacao: Passo[];
  };
}

/** Detecta os clientes de IA (pode levar alguns segundos). */
export function estadoClientes(): Promise<EstadoClientes> {
  return invoke<EstadoClientes>("estado_clientes");
}
