import { useCallback, useEffect, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, BarraAcoes, type Acao } from "./componentes";
import type { ApiConhecimento, ResumoRevalidacao } from "./conhecimento";
import { contar } from "./formatacao";
import { MudancasCodigo } from "./MudancasCodigo";

type Revalidacao =
  | { tipo: "conferindo" }
  | { tipo: "feita"; resumo: ResumoRevalidacao; em: Date }
  | { tipo: "falha"; mensagem: string };

const hora = (d: Date) => d.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });

/**
 * Conferência das evidências no cabeçalho da Base (#328, §9.4): confere as regras
 * contra o código atual ao abrir (spec, seção 6.5), diz quando foi, deixa
 * "Conferir de novo" e abre "O que mudou no código". `aoMudar` avisa quando a
 * conferência mudou a situação de alguma regra, para a lista ser relida.
 */
export function ConferenciaEvidencias({
  api,
  projeto,
  aoMudar,
}: {
  api: ApiConhecimento;
  projeto: string;
  aoMudar: () => void;
}) {
  const [revalidacao, setRevalidacao] = useState<Revalidacao>({ tipo: "conferindo" });
  const [mudancas, setMudancas] = useState(false);

  const revalidar = useCallback(() => {
    setRevalidacao({ tipo: "conferindo" });
    api
      .revalidar(projeto)
      .then((resumo) => {
        setRevalidacao({ tipo: "feita", resumo, em: new Date() });
        if (resumo.mudou) aoMudar();
      })
      .catch((e: unknown) => setRevalidacao({ tipo: "falha", mensagem: mensagemDe(e) }));
  }, [api, projeto, aoMudar]);

  // Ao abrir (e ao trocar de projeto): a conferência em seguida.
  useEffect(() => {
    revalidar();
    // `aoMudar` troca a cada render do pai; só o projeto e a api disparam de novo.
  }, [api, projeto]);

  const acoes: Acao[] = [
    { texto: "Conferir de novo", aoEscolher: revalidar, desabilitado: revalidacao.tipo === "conferindo" },
    ...(api.mudancas && !mudancas
      ? [{ texto: "O que mudou no código", aoEscolher: () => setMudancas(true) }]
      : []),
  ];

  return (
    <>
      {revalidacao.tipo === "falha" && (
        <Alerta tom="erro" titulo="Não foi possível conferir as evidências.">
          {revalidacao.mensagem}
        </Alerta>
      )}
      <div className="revalidacao" role="status" aria-label="Revalidação">
        {revalidacao.tipo === "conferindo" && (
          <span className="muted">Conferindo as evidências com o código atual…</span>
        )}
        {revalidacao.tipo === "feita" && (
          <TextoRevalidacao resumo={revalidacao.resumo} em={revalidacao.em} />
        )}
        <BarraAcoes secundarias={acoes} rotulo="Conferência das evidências" />
      </div>
      {api.mudancas && mudancas && (
        <MudancasCodigo api={api.mudancas} projeto={projeto} aoFechar={() => setMudancas(false)} />
      )}
    </>
  );
}

function TextoRevalidacao({ resumo, em }: { resumo: ResumoRevalidacao; em: Date }) {
  const marcadas = resumo.regras.filter((r) => r.depois === "a-revalidar");
  const restauradas = resumo.regras.filter((r) => r.depois === "confirmada");
  const partes = [
    `${contar(resumo.evidencias, "evidência conferida", "evidências conferidas")} às ${hora(em)}`,
  ];
  if (resumo.desatualizadas > 0) partes.push(`${resumo.desatualizadas} não confere(m)`);
  if (marcadas.length > 0)
    partes.push(`passaram a revalidar: ${marcadas.map((r) => r.id).join(", ")}`);
  if (restauradas.length > 0)
    partes.push(`voltaram a confirmadas: ${restauradas.map((r) => r.id).join(", ")}`);
  const historias = resumo.historiasAfetadas ?? 0;
  const cenarios = (resumo.cenariosAfetados ?? 0) + (resumo.tabelasAfetadas ?? 0);
  if (historias > 0)
    partes.push(`${contar(historias, "história", "histórias")} com critério a revalidar`);
  if (cenarios > 0)
    partes.push(`${contar(cenarios, "cenário ou tabela", "cenários e tabelas")} a revalidar`);
  if (resumo.problemas.length > 0)
    partes.push(`${contar(resumo.problemas.length, "documento", "documentos")} com problema`);
  return <span className="muted">{partes.join("; ")}.</span>;
}
