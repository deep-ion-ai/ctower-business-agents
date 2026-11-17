import { useEffect, useState } from "react";
import type { ApiAssistenteProjeto } from "./estadoCadastro";

/**
 * Se há token global guardado no chaveiro para cada host: `true`/`false`, `undefined`
 * enquanto consulta e `null` se a consulta falhou. Nunca traz o token.
 */
export function useTokensGuardados(
  api: Pick<ApiAssistenteProjeto, "tokensGuardados">,
  hosts: string[],
): Record<string, boolean | null | undefined> {
  const chave = hosts.join(" ");
  const [guardados, setGuardados] = useState<Record<string, boolean | null>>({});
  useEffect(() => {
    const lista = chave ? chave.split(" ") : [];
    if (lista.length === 0) return;
    let ativo = true;
    Promise.resolve()
      .then(() => api.tokensGuardados(lista.map((host) => ({ host, caminho: null }))))
      .then((r) => {
        if (ativo) setGuardados(Object.fromEntries(lista.map((h, i) => [h, r?.[i] === true])));
      })
      .catch(() => {
        if (ativo) setGuardados(Object.fromEntries(lista.map((h) => [h, null])));
      });
    return () => {
      ativo = false;
    };
  }, [api, chave]);
  return guardados;
}
