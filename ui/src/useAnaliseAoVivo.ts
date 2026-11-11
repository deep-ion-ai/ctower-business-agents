import { useCallback, useEffect, useRef, useState } from "react";
import { mensagemDe, type ApiAnalise, type VistaAnalise } from "./analise";

export interface AnaliseAoVivo {
  vista: VistaAnalise | null;
  erro: string | null;
  recarregar: () => void;
}

/**
 * Estado da análise, relido a cada aviso da vigia do diário de eventos sobre ela (ou a
 * cada reinício da vigia). Releituras pedidas enquanto outra está em curso viram uma
 * só, feita logo depois, para a tela nunca ficar com uma versão anterior à do aviso.
 */
export function useAnaliseAoVivo(api: ApiAnalise, sistema: string, analise: string): AnaliseAoVivo {
  const [vista, setVista] = useState<VistaAnalise | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const versao = useRef(-1);
  const carregando = useRef(false);
  const pendente = useRef(false);
  const ativo = useRef(true);

  const carregar = useCallback(() => {
    if (carregando.current) {
      pendente.current = true;
      return;
    }
    carregando.current = true;
    api
      .carregarAnalise(sistema, analise)
      .then((v) => {
        if (!ativo.current) return;
        // Uma resposta mais velha que a já mostrada (corrida entre releituras) é ignorada.
        if (v.analise.versao >= versao.current) {
          versao.current = v.analise.versao;
          setVista(v);
        }
        setErro(null);
      })
      .catch((e: unknown) => ativo.current && setErro(mensagemDe(e)))
      .finally(() => {
        carregando.current = false;
        if (pendente.current && ativo.current) {
          pendente.current = false;
          carregar();
        }
      });
  }, [api, sistema, analise]);

  useEffect(() => {
    ativo.current = true;
    versao.current = -1;
    setVista(null);
    setErro(null);
    let parar: (() => void) | null = null;
    let desmontado = false;
    // Assina antes de ler, para nenhuma mudança cair entre a leitura e a assinatura.
    api
      .vigiar(sistema, (aviso) => {
        if (aviso.tipo === "reinicio") carregar();
        else if (aviso.analise === analise && aviso.versao > versao.current) carregar();
      })
      .then((f) => {
        if (desmontado) f();
        else parar = f;
      })
      .catch(() => undefined)
      .finally(() => {
        if (!desmontado) carregar();
      });
    return () => {
      desmontado = true;
      ativo.current = false;
      parar?.();
    };
  }, [api, sistema, analise, carregar]);

  return { vista, erro, recarregar: carregar };
}
