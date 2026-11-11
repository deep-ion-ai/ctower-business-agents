import { useCallback, useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { mensagemDe } from "./analise";
import { AssistentePrimeiroUso, type FimPrimeiroUso } from "./AssistentePrimeiroUso";
import { ClientesIA } from "./ClientesIA";
import { apiPrimeiroUsoTauri, type ApiPrimeiroUso, type Situacao } from "./primeiroUso";
import { TelaAnalise } from "./TelaAnalise";

export const PRODUTO = "Ctower Business Agents";

/**
 * Tela inicial: nome do produto e versão do app. No primeiro uso, o assistente
 * (cliente de IA, base, sistema e ambiente); depois, o estado dos clientes de IA e a
 * pergunta ao sistema, com a tela da análise (terminal e documento ao vivo).
 */
export function App({ api = apiPrimeiroUsoTauri }: { api?: ApiPrimeiroUso }) {
  const [versao, setVersao] = useState<string | null>(null);
  const [situacao, setSituacao] = useState<Situacao | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [assistente, setAssistente] = useState(false);
  const [inicio, setInicio] = useState<FimPrimeiroUso | null>(null);

  useEffect(() => {
    let ativo = true;
    getVersion()
      .then((v) => ativo && setVersao(v))
      // Fora do Tauri (ex.: `npm run dev` no navegador) não há versão.
      .catch(() => ativo && setVersao(null));
    return () => {
      ativo = false;
    };
  }, []);

  const carregar = useCallback(() => {
    api
      .situacao()
      .then((s) => {
        setSituacao(s);
        setFalha(null);
        setAssistente(!s.primeiroUsoConcluido);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api]);

  useEffect(carregar, [carregar]);

  /** Reabre o assistente com a situação atual (cada passo pode ser refeito). */
  const reabrir = () => {
    api
      .situacao()
      .then((s) => {
        setSituacao(s);
        setFalha(null);
        setAssistente(true);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  const concluir = (fim: FimPrimeiroUso) => {
    setInicio(fim);
    setAssistente(false);
    carregar();
  };

  return (
    <main className="inicio">
      <h1>{PRODUTO}</h1>
      <p className="versao" aria-label="Versão">
        {versao ? `Versão ${versao}` : " "}
      </p>
      {falha && (
        <p className="trecho-erro" role="alert">
          Não foi possível ler a configuração do app: {falha}
        </p>
      )}
      {situacao && assistente && (
        <AssistentePrimeiroUso api={api} situacao={situacao} aoConcluir={concluir} />
      )}
      {(falha || (situacao && !assistente)) && (
        <>
          <ClientesIA abrirLink={api.abrirLink} />
          <TelaAnalise
            key={inicio?.sistema ?? ""}
            sistemaInicial={inicio?.sistema}
            perguntaInicial={inicio?.pergunta}
            aoPrimeiroUso={reabrir}
          />
          {situacao && (
            <p>
              <button type="button" className="link" onClick={reabrir}>
                Refazer o primeiro uso
              </button>
            </p>
          )}
        </>
      )}
    </main>
  );
}
