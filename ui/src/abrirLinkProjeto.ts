import { invoke } from "@tauri-apps/api/core";

/**
 * Abre no navegador do sistema um link de forja ou de rastreador do projeto (#398):
 * a issue, o PR, a linha da evidência. O app só abre o host de um repositório
 * cadastrado no projeto; outro host é recusado com o motivo em português.
 */
export function abrirLinkDoProjeto(projeto: string, link: string): Promise<void> {
  return invoke("abrir_link_do_projeto_app", { projeto, link });
}
