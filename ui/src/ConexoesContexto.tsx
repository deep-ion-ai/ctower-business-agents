import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { mensagemDe } from "./analise";
import type { ApiConexoes, ConexaoVista, Descritor } from "./conexoes";

/**
 * As conexões e os descritores, lidos uma vez pela tela que cadastra projetos (as
 * Configurações e o assistente de projeto) e compartilhados pelos seletores de
 * conexão dos formulários (#437). Sem o provedor, os seletores não aparecem e os
 * formulários seguem como antes.
 */
export interface EstadoConexoes {
  api: ApiConexoes;
  descritores: Descritor[];
  conexoes: ConexaoVista[];
  carregando: boolean;
  falha: string | null;
  recarregar: () => Promise<void>;
}

const Contexto = createContext<EstadoConexoes | null>(null);

export function ProvedorConexoes({ api, children }: { api: ApiConexoes; children: ReactNode }) {
  const [descritores, setDescritores] = useState<Descritor[]>([]);
  const [conexoes, setConexoes] = useState<ConexaoVista[]>([]);
  const [carregando, setCarregando] = useState(true);
  const [falha, setFalha] = useState<string | null>(null);

  const recarregar = useCallback(async () => {
    try {
      const [d, c] = await Promise.all([api.descritoresConexao(), api.listarConexoes()]);
      setDescritores(Array.isArray(d) ? d : []);
      setConexoes(Array.isArray(c) ? c : []);
      setFalha(null);
    } catch (e) {
      setFalha(mensagemDe(e));
    } finally {
      setCarregando(false);
    }
  }, [api]);

  useEffect(() => {
    void recarregar();
  }, [recarregar]);

  const valor = useMemo(
    () => ({ api, descritores, conexoes, carregando, falha, recarregar }),
    [api, descritores, conexoes, carregando, falha, recarregar],
  );
  return <Contexto.Provider value={valor}>{children}</Contexto.Provider>;
}

/** As conexões do provedor; `null` fora dele. */
export function useConexoes(): EstadoConexoes | null {
  return useContext(Contexto);
}

/** Provê as conexões, a menos que uma tela de fora já as proveja. */
export function ComConexoes({ api, children }: { api: ApiConexoes; children: ReactNode }) {
  const ctx = useConexoes();
  return ctx ? <>{children}</> : <ProvedorConexoes api={api}>{children}</ProvedorConexoes>;
}
