import { formatar, type Formato } from "./formatacao";

/**
 * Insere `texto` na posição do cursor do `textarea` (ou no lugar da seleção) e deixa o
 * cursor depois dele. Usa `document.execCommand("insertText")`, que entra na pilha de
 * desfazer do navegador (Ctrl+Z desfaz a inserção); sem ele (ou se recusar), cai em
 * `setRangeText` e avisa o React com um evento `input`, sem o desfazer nativo.
 *
 * Reutilizável por qualquer ação do editor (variáveis, barra de formatação).
 */
export function inserirNoCursor(campo: HTMLTextAreaElement, texto: string): void {
  campo.focus();
  let feito = false;
  try {
    feito =
      typeof document.execCommand === "function" &&
      document.execCommand("insertText", false, texto);
  } catch {
    feito = false;
  }
  if (feito) return;
  campo.setRangeText(texto, campo.selectionStart, campo.selectionEnd, "end");
  campo.dispatchEvent(new Event("input", { bubbles: true }));
}

/** Seleciona a linha (a partir de 1) e a traz para a vista. */
export function irParaLinha(campo: HTMLTextAreaElement, linha: number): void {
  const linhas = campo.value.split("\n");
  const alvo = Math.min(Math.max(linha, 1), linhas.length);
  let inicio = 0;
  for (let i = 0; i < alvo - 1; i++) inicio += linhas[i].length + 1;
  campo.focus();
  campo.setSelectionRange(inicio, inicio + linhas[alvo - 1].length);
  const altura = parseFloat(getComputedStyle(campo).lineHeight);
  if (Number.isFinite(altura)) {
    campo.scrollTop = Math.max(0, (alvo - 1) * altura - campo.clientHeight / 3);
  }
}

/**
 * Troca o trecho `[inicio, fim)` por `texto` (selecionando-o antes, para que a troca
 * entre no desfazer como uma só inserção) e deixa `[selInicio, selFim)` selecionado.
 */
export function substituirTrecho(
  campo: HTMLTextAreaElement,
  inicio: number,
  fim: number,
  texto: string,
  selInicio: number,
  selFim: number,
): void {
  campo.focus();
  campo.setSelectionRange(inicio, fim);
  inserirNoCursor(campo, texto);
  campo.setSelectionRange(selInicio, selFim);
}

/** Aplica um formato da barra à seleção do campo (ou insere o modelo sem seleção). */
export function aplicarFormatoNoCampo(campo: HTMLTextAreaElement, formato: Formato): void {
  const e = formatar(formato, campo.value, campo.selectionStart, campo.selectionEnd);
  substituirTrecho(campo, e.inicio, e.fim, e.texto, e.selInicio, e.selFim);
}
