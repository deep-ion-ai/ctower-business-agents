import { useCallback, useEffect, useState } from "react";
import { mensagemDe, type ApiAnalise } from "./analise";
import type { AbrirLink } from "./ClientesIA";
import { apiConhecimentoTauri, type ApiConhecimento } from "./conhecimento";
import type { Abertura } from "./PainelAnalise";
import { TelaAnalise } from "./TelaAnalise";
import type { ApiConfiguracoes } from "./configuracoes";
import { TelaBase } from "./TelaBase";
import { TelaConfiguracoes } from "./TelaConfiguracoes";
import { TelaInicio } from "./TelaInicio";
import { TelaSistema } from "./TelaSistema";
import type { SistemaResumo } from "./terminal";

/** Tela visível e o que ela abre. */
export type Tela =
  | { tipo: "inicio" }
  | { tipo: "sistema"; sistema?: string }
  | { tipo: "base"; documento?: string }
  | {
      tipo: "analise";
      sistema?: string;
      pergunta?: string;
      aberta?: { id: string; abertura: Abertura };
    }
  | { tipo: "configuracoes" };

interface Props {
  /** Tela ao abrir (ex.: a análise sugerida no fim do primeiro uso). */
  inicial?: Tela;
  api?: ApiConhecimento;
  /** Para testes. */
  apiAnalise?: ApiAnalise;
  /** Para testes. */
  apiConfiguracoes?: ApiConfiguracoes;
  abrirLink?: AbrirLink;
  /** Abre o assistente do primeiro uso. */
  aoPrimeiroUso?: () => void;
}

const ABAS: { tipo: Tela["tipo"]; rotulo: string }[] = [
  { tipo: "inicio", rotulo: "Início" },
  { tipo: "sistema", rotulo: "Sistema" },
  { tipo: "base", rotulo: "Base de conhecimento" },
  { tipo: "analise", rotulo: "Análises" },
  { tipo: "configuracoes", rotulo: "Configurações" },
];

/**
 * Navegação entre as telas depois do primeiro uso (spec, seção 10): início, sistema,
 * base de conhecimento e análises. Cada tela pede às outras o que abrir (um sistema,
 * um documento, uma análise) por `navegar`.
 */
export function Navegacao({
  inicial = { tipo: "inicio" },
  api = apiConhecimentoTauri,
  apiAnalise,
  apiConfiguracoes,
  abrirLink,
  aoPrimeiroUso,
}: Props) {
  const [tela, setTela] = useState<Tela>(inicial);
  // Muda a cada navegação, para a tela de destino abrir do zero com o pedido.
  const [vez, setVez] = useState(0);
  const [sistemas, setSistemas] = useState<SistemaResumo[]>([]);
  const [falha, setFalha] = useState<string | null>(null);
  // Muda quando as configurações mexem nos sistemas ou na base.
  const [recarga, setRecarga] = useState(0);

  const navegar = useCallback((t: Tela) => {
    setTela(t);
    setVez((v) => v + 1);
  }, []);

  useEffect(() => {
    let ativo = true;
    api
      .situacao()
      .then((s) => ativo && setSistemas(s.sistemas))
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, vez, recarga]);

  return (
    <div className="navegacao">
      <nav className="abas" aria-label="Telas">
        {ABAS.map((a) => (
          <button
            key={a.tipo}
            type="button"
            aria-current={tela.tipo === a.tipo ? "page" : undefined}
            onClick={() => navegar({ tipo: a.tipo } as Tela)}
          >
            {a.rotulo}
          </button>
        ))}
      </nav>
      {falha && tela.tipo !== "inicio" && (
        <p className="trecho-erro" role="alert">
          {falha}
        </p>
      )}
      {tela.tipo === "inicio" && (
        <TelaInicio
          key={vez}
          api={api}
          navegar={navegar}
          abrirLink={abrirLink}
          aoPrimeiroUso={aoPrimeiroUso}
        />
      )}
      {tela.tipo === "sistema" && (
        <TelaSistema
          key={vez}
          api={api}
          sistemas={sistemas}
          sistemaInicial={tela.sistema}
          navegar={navegar}
        />
      )}
      {tela.tipo === "base" && (
        <TelaBase
          key={vez}
          api={api}
          sistemas={sistemas}
          documentoInicial={tela.documento}
          navegar={navegar}
        />
      )}
      {tela.tipo === "analise" && (
        <TelaAnalise
          key={vez}
          api={apiAnalise}
          sistemaInicial={tela.sistema}
          perguntaInicial={tela.pergunta}
          aberturaInicial={tela.aberta}
          aoPrimeiroUso={aoPrimeiroUso}
        />
      )}
      {tela.tipo === "configuracoes" && (
        <TelaConfiguracoes
          key={vez}
          api={apiConfiguracoes}
          aoMudarSistemas={() => setRecarga((r) => r + 1)}
        />
      )}
    </div>
  );
}
