import { useCallback, useEffect, useRef, useState } from "react";
import { mensagemDe, type ApiAnalise, type VistaAnalise } from "./analise";

/**
 * Intervalo da conferência das evidências enquanto a análise está aberta (#216): uma
 * edição no código citado vira a faixa "O código citado na análise mudou" em até
 * esse tempo (mais o da vigia). Sem mudança nos arquivos, cada conferência é só a
 * leitura do estado e a consulta do tamanho e da data dos arquivos citados.
 */
export const INTERVALO_DAS_EVIDENCIAS_MS = 2000;

export interface AnaliseAoVivo {
  vista: VistaAnalise | null;
  erro: string | null;
  recarregar: () => void;
}

/**
 * Estado da análise, relido a cada aviso da vigia do diário de eventos sobre ela (ou a
 * cada reinício da vigia). Releituras pedidas enquanto outra está em curso viram uma
 * só, feita logo depois, para a tela nunca ficar com uma versão anterior à do aviso.
 *
 * Enquanto a tela está montada, confere também as evidências da análise contra o código
 * a cada {@link INTERVALO_DAS_EVIDENCIAS_MS}: a edição feita fora da sessão (no editor,
 * por um `git pull`) marca as afirmações a revalidar, e o aviso da vigia traz a análise
 * nova. Uma conferência não começa antes de a anterior terminar.
 */
export function useAnaliseAoVivo(api: ApiAnalise, projeto: string, analise: string): AnaliseAoVivo {
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
      .carregarAnalise(projeto, analise)
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
  }, [api, projeto, analise]);

  useEffect(() => {
    ativo.current = true;
    versao.current = -1;
    setVista(null);
    setErro(null);
    let parar: (() => void) | null = null;
    let desmontado = false;
    // Assina antes de ler, para nenhuma mudança cair entre a leitura e a assinatura.
    api
      .vigiar(projeto, (aviso) => {
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
  }, [api, projeto, analise, carregar]);

  useEffect(() => {
    let conferindo = false;
    const conferir = () => {
      if (conferindo) return;
      conferindo = true;
      api
        .conferirEvidencias(projeto, analise)
        // O aviso da vigia também relê; reler aqui cobre a vigia que não assinou.
        .then((mudou) => {
          if (mudou && ativo.current) carregar();
        })
        // Falha (projeto fora do cadastro, base inacessível) já aparece na leitura.
        .catch(() => undefined)
        .finally(() => {
          conferindo = false;
        });
    };
    conferir();
    const id = setInterval(conferir, INTERVALO_DAS_EVIDENCIAS_MS);
    return () => clearInterval(id);
  }, [api, projeto, analise, carregar]);

  return { vista, erro, recarregar: carregar };
}
