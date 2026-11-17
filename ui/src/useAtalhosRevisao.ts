import { useEffect, useRef } from "react";

export interface AtalhosRevisao {
  aoProxima: () => void;
  aoAnterior: () => void;
  aoConfirmar: () => void;
  aoIncorreta: () => void;
  aoAprofundar: () => void;
}

/**
 * O foco está num lugar onde a tecla é do usuário: campo de texto, o terminal (o
 * xterm digita num textarea escondido) ou um diálogo aberto.
 */
export function focoEmCampoOuTerminal(alvo: EventTarget | null): boolean {
  if (!(alvo instanceof HTMLElement)) return false;
  if (alvo.isContentEditable) return true;
  if (["INPUT", "TEXTAREA", "SELECT"].includes(alvo.tagName)) return true;
  return !!alvo.closest(".xterm, .terminal, [role='dialog'], dialog, [contenteditable='true']");
}

/**
 * Atalhos da revisão (#184): J/K navegam entre as afirmações, C confirma, X marca
 * incorreta e A pede aprofundar. Não disparam com o foco em campo de texto, no
 * terminal ou num diálogo, nem com Ctrl, Alt ou Cmd apertados.
 */
export function useAtalhosRevisao(ativo: boolean, atalhos: AtalhosRevisao) {
  const ref = useRef(atalhos);
  ref.current = atalhos;
  useEffect(() => {
    if (!ativo) return;
    const aoTeclar = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey) return;
      if (focoEmCampoOuTerminal(e.target) || focoEmCampoOuTerminal(document.activeElement)) return;
      const a = ref.current;
      const tecla = e.key.toLowerCase();
      // J e K aceitam repetição (segurar a tecla); as ações, não.
      if (tecla === "j") a.aoProxima();
      else if (tecla === "k") a.aoAnterior();
      else if (e.repeat) return;
      else if (tecla === "c") a.aoConfirmar();
      else if (tecla === "x") a.aoIncorreta();
      else if (tecla === "a") a.aoAprofundar();
      else return;
      e.preventDefault();
    };
    document.addEventListener("keydown", aoTeclar);
    return () => document.removeEventListener("keydown", aoTeclar);
  }, [ativo]);
}
