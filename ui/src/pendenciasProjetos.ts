import { useCallback, useEffect, useRef, useState } from "react";
import type { AmbienteDetalhado, ApiConfiguracoes, ProjetoConfig } from "./configuracoes";
import type { ProjetoNoMenu } from "./MenuConfiguracoes";
import type { Prontidao } from "./repositorios";
import { pendenciasDoProjeto, type EstadoDoProjeto } from "./saudeProjeto";

/**
 * O que se sabe de cada projeto nas configurações (#191, #192): o ambiente do Claude
 * e a prontidão dos repositórios. O ambiente é verificado uma vez por abertura da tela,
 * para todos os projetos (o menu mostra as pendências); a página do projeto reaproveita
 * esse resultado em vez de verificar de novo. A prontidão (que acessa a rede) só é
 * conferida quando a página do projeto abre. Depois de uma ação que muda o ambiente
 * (preparar, desfazer, trocar as pastas), a página pede a verificação de novo.
 */
export function useEstadoProjetos(
  api: Pick<ApiConfiguracoes, "verificarAmbiente" | "prontidaoRepositorios">,
  projetos: ProjetoConfig[],
) {
  const [ambientes, setAmbientes] = useState<Record<string, AmbienteDetalhado | null>>({});
  const [prontidoes, setProntidoes] = useState<Record<string, Prontidao | null>>({});
  // Projetos já pedidos nesta abertura: a lista de projetos muda de identidade a cada
  // releitura das configurações, e isso não deve repetir a verificação.
  const pedidos = useRef(new Set<string>());
  const pedidasProntidoes = useRef(new Set<string>());

  const reverificarAmbiente = useCallback(
    (id: string) =>
      api
        .verificarAmbiente(id)
        .then((a) => {
          setAmbientes((m) => ({ ...m, [id]: a }));
          return a;
        })
        .catch(() => {
          setAmbientes((m) => ({ ...m, [id]: null }));
          return null;
        }),
    [api],
  );

  const conferirProntidao = useCallback(
    (id: string) => {
      pedidasProntidoes.current.add(id);
      setProntidoes((m) => {
        const { [id]: _, ...resto } = m;
        return resto;
      });
      return api
        .prontidaoRepositorios(id)
        .then((p) => setProntidoes((m) => ({ ...m, [id]: p })))
        .catch(() => setProntidoes((m) => ({ ...m, [id]: null })));
    },
    [api],
  );

  /** Confere a prontidão só se ainda não foi conferida nesta abertura. */
  const garantirProntidao = useCallback(
    (id: string) => {
      if (!pedidasProntidoes.current.has(id)) void conferirProntidao(id);
    },
    [conferirProntidao],
  );

  useEffect(() => {
    for (const p of projetos) {
      if (pedidos.current.has(p.id)) continue;
      pedidos.current.add(p.id);
      void reverificarAmbiente(p.id);
    }
  }, [projetos, reverificarAmbiente]);

  const estadoDe = useCallback(
    (id: string): EstadoDoProjeto => ({
      ambiente: id in ambientes ? ambientes[id] : undefined,
      prontidao: id in prontidoes ? prontidoes[id] : undefined,
    }),
    [ambientes, prontidoes],
  );

  const noMenu: ProjetoNoMenu[] = projetos.map((p) => ({
    id: p.id,
    nome: p.nome,
    pendencias: pendenciasDoProjeto(p, estadoDe(p.id)),
  }));

  return { noMenu, estadoDe, reverificarAmbiente, conferirProntidao, garantirProntidao };
}
