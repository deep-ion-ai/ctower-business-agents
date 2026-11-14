import { useEffect, useState } from "react";
import { mensagemDe } from "./analise";
import { DialogoConfirmacao } from "./DialogoConfirmacao";
import type { ApiReuniao, AtaAprovada, ProntidaoAta } from "./reunioes";

interface Props {
  api: ApiReuniao;
  projeto: string;
  /** A análise do fluxo `reuniao`. */
  analise: string;
  /** Versão da análise: a prontidão é relida a cada mudança. */
  versao: number;
  /** Chamado depois de aprovar, para a tela reler a análise. */
  aoAprovar?: (feita: AtaAprovada) => void;
}

/**
 * Aprovação da ata de uma reunião (#142), na tela de revisão no lugar da aprovação
 * das análises: mostra o que falta, os avisos e, ao aprovar, o **segundo aviso** (o
 * transcript é apagado e as citações deixam de ser conferíveis), com a confirmação
 * explícita. Sem confirmar, nada é enviado: a ata continua em rascunho e o
 * transcript, guardado.
 */
export function AprovacaoAta({ api, projeto, analise, versao, aoAprovar }: Props) {
  const [prontidao, setProntidao] = useState<ProntidaoAta | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [confirmando, setConfirmando] = useState(false);
  const [ocupado, setOcupado] = useState(false);
  const [feita, setFeita] = useState<AtaAprovada | null>(null);
  const [leitura, setLeitura] = useState(0);

  useEffect(() => {
    let ativo = true;
    api
      .prontidaoAta(projeto, analise)
      .then((p) => ativo && setProntidao(p))
      .catch((e: unknown) => ativo && setErro(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, projeto, analise, versao, leitura]);

  if (!prontidao) return erro ? <p className="error-text">{erro}</p> : null;

  const aprovar = () => {
    setConfirmando(false);
    setOcupado(true);
    setErro(null);
    api
      .aprovarAta(projeto, analise, true)
      .then((f) => {
        setFeita(f);
        aoAprovar?.(f);
      })
      .catch((e: unknown) => setErro(mensagemDe(e)))
      .finally(() => {
        setOcupado(false);
        setLeitura((n) => n + 1);
      });
  };

  const incompleta = prontidao.status === "aprovada" && !prontidao.concluida;

  return (
    <section className="aprovacao aprovacao-ata" aria-label="Aprovação da ata">
      <h3>Aprovação da ata</h3>
      {prontidao.concluida && !feita && (
        <p role="status">Ata aprovada: o transcript já foi apagado.</p>
      )}
      {!prontidao.concluida && (
        <>
          {incompleta && (
            <p className="error-text" role="alert">
              A aprovação da ata ficou incompleta. Aprove de novo para concluir.
            </p>
          )}
          {prontidao.pendencias.length > 0 && (
            <div className="pendencias">
              <p>Antes de aprovar:</p>
              <ul aria-label="Pendências">
                {prontidao.pendencias.map((p) => (
                  <li key={p}>{p}</li>
                ))}
              </ul>
            </div>
          )}
          {prontidao.avisos.length > 0 && (
            <ul className="muted" aria-label="Avisos da aprovação">
              {prontidao.avisos.map((a) => (
                <li key={a}>{a}</li>
              ))}
            </ul>
          )}
          <p className="muted">
            {prontidao.itens} item(ns) na ata.
            {prontidao.transcriptGuardado
              ? " Ao aprovar, o transcript é apagado."
              : " O transcript já não está guardado."}
          </p>
          <button
            className="btn"
            type="button"
            disabled={!prontidao.podeAprovar || ocupado}
            onClick={() => setConfirmando(true)}
          >
            Aprovar a ata
          </button>
        </>
      )}
      {confirmando && (
        <DialogoConfirmacao
          titulo="Aprovar a ata e apagar o transcript?"
          aoCancelar={() => setConfirmando(false)}
          acoes={
            <>
              <button className="btn" type="button" autoFocus onClick={() => setConfirmando(false)}>
                Cancelar
              </button>
              <button className="btn btn-primary" type="button" onClick={aprovar}>
                Aprovar e apagar o transcript
              </button>
            </>
          }
        >
          <p>{prontidao.avisoDescarte}</p>
          {prontidao.citacoesConferiveis > 0 && (
            <p className="muted">
              {prontidao.citacoesConferiveis} citação(ões) deixam de ser conferíveis contra o
              transcript.
            </p>
          )}
        </DialogoConfirmacao>
      )}
      {erro && (
        <p className="error-text" role="alert">
          {erro}
        </p>
      )}
      {feita && (
        <div role="status" aria-label="Resultado da aprovação da ata">
          <p>
            Ata aprovada em {feita.data}.{" "}
            {feita.transcriptApagado
              ? "O transcript foi apagado."
              : "O transcript já tinha sido apagado."}
            {feita.citacoesMarcadas > 0 &&
              ` ${feita.citacoesMarcadas} citação(ões) marcadas como conferidas em ${feita.data}.`}
          </p>
          <p className="muted">
            As pendências, as dúvidas e os próximos passos já estão no painel da demanda.
          </p>
          {feita.avisos.map((a) => (
            <p key={a} className="muted">
              {a}
            </p>
          ))}
        </div>
      )}
    </section>
  );
}
