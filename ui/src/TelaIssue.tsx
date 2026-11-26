import { useEffect, useMemo, useState } from "react";
import { acaoDaIssue } from "./acaoIssue";
import { mensagemDe } from "./analise";
import type { ApiConhecimento } from "./conhecimento";
import {
  AvisoAbrirLink,
  DescricaoIssue,
  NaBaseIssue,
  ParecidasLista,
  ROTULO_ORIGEM,
  useAbrirLink,
  useRegrasParecidas,
} from "./DetalheIssue";
import { parecidos } from "./demandasParecidas";
import { idadeDe, type ApiIssues, type IssueDoProjeto, type OrigemIssue } from "./issuesProjeto";
import type { Tela } from "./Navegacao";

type Carga =
  | { estado: "lendo" }
  | { estado: "ok"; issue: IssueDoProjeto; todas: IssueDoProjeto[] }
  | { estado: "ausente" }
  | { estado: "falha"; mensagem: string };

/**
 * Aba de uma issue do rastreador (#398): o que a lista mostra no painel, em tela
 * inteira. A aba guarda só o projeto, a origem e a chave; a issue é relida do cache.
 * A ação e a abertura no navegador são as mesmas da lista.
 */
export function TelaIssue({
  api,
  issues,
  projeto,
  origem,
  chave,
  navegar,
}: {
  api: ApiConhecimento;
  issues: ApiIssues;
  projeto: string;
  origem: OrigemIssue;
  chave: string;
  navegar: (t: Tela) => void;
}) {
  const [carga, setCarga] = useState<Carga>({ estado: "lendo" });
  const [analisando, setAnalisando] = useState(false);
  const [falhaAcao, setFalhaAcao] = useState<string | null>(null);
  const { abrir, falha } = useAbrirLink(projeto);
  const titulo = carga.estado === "ok" ? carga.issue.titulo : "";
  const regras = useRegrasParecidas(api, projeto, titulo);

  useEffect(() => {
    let ativo = true;
    // A busca acha a issue pelo número ou pela chave; a segunda leitura dá as parecidas.
    Promise.all([
      issues.listar(projeto, { busca: chave, estado: "todas" }),
      issues.listar(projeto, { estado: "todas" }),
    ])
      .then(([achadas, todas]) => {
        if (!ativo) return;
        const issue = achadas.issues.find(
          (i) => i.origem === origem && i.chave.toLowerCase() === chave.toLowerCase(),
        );
        setCarga(issue ? { estado: "ok", issue, todas: todas.issues } : { estado: "ausente" });
      })
      .catch((e: unknown) => {
        if (ativo) setCarga({ estado: "falha", mensagem: mensagemDe(e) });
      });
    return () => {
      ativo = false;
    };
  }, [issues, projeto, origem, chave]);

  const similares = useMemo(
    () =>
      carga.estado !== "ok"
        ? []
        : parecidos(
            { id: chave, titulo: carga.issue.titulo },
            carga.todas.map((i) => ({ id: i.chave, titulo: i.titulo, linha: i })),
          ),
    [carga, chave],
  );

  if (carga.estado === "lendo") return <div className="page"><p className="muted">Carregando…</p></div>;
  if (carga.estado === "falha")
    return (
      <div className="page">
        <p className="error-text" role="alert">{carga.mensagem}</p>
      </div>
    );
  if (carga.estado === "ausente")
    return (
      <div className="page">
        <p className="muted">
          A issue {chave} não está na lista do rastreador. Use Atualizar na tela Demandas e abra a aba de novo.
        </p>
      </div>
    );

  const { issue } = carga;
  const acao = acaoDaIssue(issue);
  const abrirAnalise = (id: string) =>
    navegar({ tipo: "analise", projeto, aberta: { id, abertura: { tipo: "reabrir" } } });

  const analisar = () => {
    if (analisando) return;
    setAnalisando(true);
    setFalhaAcao(null);
    issues
      .analisarImpacto(projeto, origem, chave)
      .then((r) =>
        navegar({
          tipo: "analise",
          projeto,
          aberta: { id: r.criada.analise.id, abertura: { tipo: "nova", promptInicial: r.criada.promptInicial } },
        }),
      )
      .catch((e: unknown) => {
        setFalhaAcao(mensagemDe(e));
        setAnalisando(false);
      });
  };

  const agir = () => {
    if (acao.bloqueio) return;
    if (acao.tipo === "demanda" && issue.demanda) navegar({ tipo: "demanda", projeto, demanda: issue.demanda });
    else if (acao.tipo === "analise" && issue.analise) abrirAnalise(issue.analise);
    else if (acao.tipo === "analisar") analisar();
  };

  return (
    <div className="page">
      <section className="demandas-aba" aria-label={`Issue ${issue.chave}`}>
        <h2 className="demandas-painel-cab">
          <span className="id">{issue.chave}</span>
          <span className="badge" data-tone={issue.estado === "aberta" ? "running" : "done"}>
            {issue.estado === "aberta" ? "aberta" : "encerrada"}
          </span>
          <button type="button" className="link" onClick={() => void abrir(issue.url)}>
            Abrir no {ROTULO_ORIGEM[issue.origem]} ↗
          </button>
        </h2>
        <p className="demandas-painel-titulo">{issue.titulo}</p>
        <p className="muted">
          {[`criada ${idadeDe(issue.criadaEm)}`, ...issue.rotulos.map((r) => `rótulo ${r}`)].join(" · ")}
        </p>
        <AvisoAbrirLink falha={falha} />

        <h3>Descrição</h3>
        <DescricaoIssue issue={issue} abrir={(link) => void abrir(link)} />

        <h3>Na base</h3>
        <NaBaseIssue
          issue={issue}
          projeto={projeto}
          acao={acao}
          analisando={analisando}
          aoAgir={agir}
          navegar={navegar}
          aoAbrirAnalise={abrirAnalise}
        />
        {falhaAcao && (
          <p className="error-text" role="alert">
            {falhaAcao}
          </p>
        )}

        <ParecidasLista
          itens={similares.map((s) => ({
            chave: s.linha.chave,
            id: s.id,
            titulo: s.titulo,
            analise: s.linha.analise,
          }))}
          regras={regras}
          navegar={navegar}
        />
      </section>
    </div>
  );
}
