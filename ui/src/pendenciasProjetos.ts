import { useCallback, useEffect, useRef, useState } from "react";
import type { ApiConfiguracoes, ProjetoConfig } from "./configuracoes";
import type { ProjetoNoMenu } from "./MenuConfiguracoes";

/**
 * Pendências de cada projeto para o menu das configurações (#191): pastas que não
 * existem mais e o que falta no ambiente do Claude. `null` enquanto a verificação não
 * terminou (ou falhou): o menu mostra o ponto neutro, sem inventar “tudo pronto”.
 */
export function usePendenciasProjetos(
  api: Pick<ApiConfiguracoes, "verificarAmbiente">,
  projetos: ProjetoConfig[],
) {
  const [ambiente, setAmbiente] = useState<Record<string, number>>({});
  // Identidade estável: quem recebe `ambienteVerificado` não repete a verificação a cada mudança.
  const sabidos = useRef(ambiente);
  sabidos.current = ambiente;

  const conferir = useCallback(
    (id: string) =>
      api
        .verificarAmbiente(id)
        .then((a) =>
          setAmbiente((m) => ({ ...m, [id]: a.pendencias.length || (a.pronto ? 0 : 1) })),
        )
        .catch(() =>
          setAmbiente((m) => {
            const { [id]: _, ...resto } = m;
            return resto;
          }),
        ),
    [api],
  );

  useEffect(() => {
    projetos.forEach((p) => void conferir(p.id));
  }, [projetos, conferir]);

  /** A tela do projeto verificou o ambiente: se difere do que o menu sabe, confere de novo. */
  const ambienteVerificado = useCallback(
    (id: string, pronto: boolean) => {
      const sabido = sabidos.current[id];
      if (sabido !== undefined && (sabido === 0) !== pronto) void conferir(id);
    },
    [conferir],
  );

  const noMenu: ProjetoNoMenu[] = projetos.map((p) => ({
    id: p.id,
    nome: p.nome,
    pendencias:
      ambiente[p.id] === undefined
        ? null
        : ambiente[p.id] + p.pastas.filter((x) => !x.existe).length,
  }));

  return { noMenu, ambienteVerificado };
}
