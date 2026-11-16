import { invoke } from "@tauri-apps/api/core";

/**
 * Ambiente de teste do projeto: a URL da aplicação em teste, os usuários de teste e
 * os perfis de cada um, para simular o uso. Nesta etapa, só aplicações web com login
 * por usuário e senha. Espelha `ui/src-tauri/src/ambiente_teste.rs`. A senha vai da
 * tela para o chaveiro do sistema e nunca volta: a tela só sabe quem tem senha.
 */

export type TipoAplicacao = "web";
export type Autenticacao = "usuario_senha";

export interface UsuarioTeste {
  /** Identificador estável (`u1`, `u2`…), dado pelo app ao salvar. */
  id: string;
  usuario: string;
  perfil: string;
  descricao?: string;
}

export interface AmbienteTeste {
  url: string;
  tipo: TipoAplicacao;
  autenticacao: Autenticacao;
  observacoes?: string;
  usuarios?: UsuarioTeste[];
}

/** Usuário como a tela envia ao salvar. */
export interface EntradaUsuarioTeste {
  /** `null` num usuário novo. */
  id: string | null;
  usuario: string;
  perfil: string;
  descricao: string;
  /** Senha nova; `null` ou vazia mantém a guardada. */
  senha: string | null;
}

export interface EntradaAmbienteTeste {
  url: string;
  observacoes: string;
  usuarios: EntradaUsuarioTeste[];
}

export interface SituacaoAmbienteTeste {
  ambiente: AmbienteTeste | null;
  /** Identificadores dos usuários com senha guardada no chaveiro. */
  comSenha: string[];
}

export interface ApiAmbienteTeste {
  ambienteTeste(projeto: string): Promise<SituacaoAmbienteTeste>;
  /** Grava URL, usuários e perfis; as senhas informadas vão para o chaveiro. */
  salvarAmbienteTeste(
    projeto: string,
    ambiente: EntradaAmbienteTeste,
  ): Promise<SituacaoAmbienteTeste>;
  /** Tira o ambiente do projeto e apaga as senhas. Só depois da confirmação. */
  removerAmbienteTeste(projeto: string): Promise<SituacaoAmbienteTeste>;
}

export const apiAmbienteTesteTauri: ApiAmbienteTeste = {
  ambienteTeste: (projeto) => invoke("ambiente_teste_app", { projeto }),
  salvarAmbienteTeste: (projeto, ambiente) =>
    invoke("salvar_ambiente_teste_app", { projeto, ambiente }),
  removerAmbienteTeste: (projeto) => invoke("remover_ambiente_teste_app", { projeto }),
};

/** Por que a URL não serve, ou `null`. Repete a validação do app para avisar antes. */
export function problemaDaUrl(url: string): string | null {
  const u = url.trim();
  if (!u) return "Informe a URL do ambiente de teste.";
  if (/\s/.test(u)) return "A URL não pode ter espaços.";
  const m = /^(https?):\/\/([^/?#]*)/i.exec(u);
  if (!m) return "A URL precisa começar com https:// ou http://.";
  if (m[2].includes("@"))
    return "Tire o usuário e a senha da URL: cadastre-os na lista de usuários de teste.";
  if (!m[2].replace(/:\d+$/, "")) return "A URL precisa ter o endereço do servidor.";
  return null;
}
