import { useEffect, useState } from "react";
import type { Fluxo } from "./analise";
import type { ApiConhecimento } from "./conhecimento";

/**
 * Os fluxos de análise que o projeto não oferece (#465): Histórias e Cenários geram
 * tipos do catálogo de entregáveis e só aparecem quando habilitados em Configurações ›
 * Entregáveis. As análises já gravadas continuam abrindo. Sem a API (testes) ou se a
 * leitura falha, nada fica oculto: o app recusa ao criar.
 */
export function useFluxosOcultos(
  api: Pick<ApiConhecimento, "fluxosOcultos"> | undefined,
  projeto: string | null | undefined,
): Fluxo[] {
  const [ocultos, setOcultos] = useState<Fluxo[]>([]);
  useEffect(() => {
    setOcultos([]);
    const ler = api?.fluxosOcultos;
    if (!ler || !projeto) return;
    let vivo = true;
    Promise.resolve()
      .then(() => ler.call(api, projeto))
      .then((f) => {
        if (vivo && Array.isArray(f)) setOcultos(f);
      })
      .catch(() => {});
    return () => {
      vivo = false;
    };
  }, [api, projeto]);
  return ocultos;
}
