import { useCallback, useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { mensagemDe } from "./analise";
import { AssistentePrimeiroUso, type FimPrimeiroUso } from "./AssistentePrimeiroUso";
import { apiPrimeiroUsoTauri, type ApiPrimeiroUso, type Situacao } from "./primeiroUso";
import { Navegacao } from "./Navegacao";
import { Titlebar } from "./shell/Titlebar";

export const PRODUTO = "Ctower Business Agents";

/**
 * Raiz do app, no shell do Control Tower (#85): nome do produto e versão na barra de
 * título. No primeiro uso, o assistente
 * (cliente de IA, base, sistema e ambiente); depois, a navegação entre o início, o
 * sistema, a base de conhecimento e as análises ({@link Navegacao}). Ao concluir o
 * assistente, abre a análise com a pergunta sugerida.
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

  const avisoFalha = falha && (
    <div className="banner" role="alert">
      Não foi possível ler a configuração do app: {falha}
    </div>
  );

  if (falha || (situacao && !assistente)) {
    return (
      <Navegacao
        key={inicio?.sistema ?? ""}
        inicial={
          inicio ? { tipo: "analise", sistema: inicio.sistema, pergunta: inicio.pergunta } : undefined
        }
        abrirLink={api.abrirLink}
        aoPrimeiroUso={reabrir}
        refazerPrimeiroUso={situacao ? reabrir : undefined}
        produto={PRODUTO}
        versao={versao}
        banner={avisoFalha}
      />
    );
  }

  // Primeiro uso (ou carregando): sem barra de atividades, só a barra de título.
  return (
    <div className="shell">
      <Titlebar produto={PRODUTO} versao={versao} />
      <main className="primeiro-uso">
        {situacao && assistente && (
          <AssistentePrimeiroUso api={api} situacao={situacao} aoConcluir={concluir} />
        )}
      </main>
    </div>
  );
}
