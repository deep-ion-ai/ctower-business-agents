import { useEffect, useId, useState } from "react";
import { Alerta, Selo } from "./componentes";
import { mensagemDe } from "./analise";
import {
  descreverOrigem,
  pendentes,
  quando,
  ROTULO_SITUACAO,
  type ApiConsultasBanco,
  type BancoDoProjeto,
  type BancoLigado,
  type ConsultaDados,
  type DecisaoConsulta,
} from "./consultasBanco";

/**
 * Banco de dados na análise (#440): "Usar o banco <nome> nesta análise", as consultas
 * do agente que aguardam a aprovação (Executar, Editar e executar ou Recusar com o
 * motivo, que volta ao agente) e o registro de todas as consultas, sem os valores.
 *
 * Sem conexão de banco no projeto, não mostra nada: a análise segue só com o código.
 */
export function PainelBancoAnalise({
  api,
  projeto,
  analise,
  aprovada,
  banco,
  consultas,
}: {
  api: ApiConsultasBanco;
  projeto: string;
  analise: string;
  /** Análise aprovada: só o registro, sem ligar nem decidir. */
  aprovada: boolean;
  banco: BancoLigado | null | undefined;
  consultas: ConsultaDados[] | undefined;
}) {
  const [bancos, setBancos] = useState<BancoDoProjeto[] | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);

  useEffect(() => {
    let ativo = true;
    // Sem o comando (testes de outras telas), fica sem bancos.
    Promise.resolve()
      .then(() => api.bancosProjeto(projeto))
      .then((b) => ativo && setBancos(Array.isArray(b) ? b : []))
      .catch(() => ativo && setBancos([]));
    return () => {
      ativo = false;
    };
  }, [api, projeto]);

  const lista = consultas ?? [];
  if (!bancos || (bancos.length === 0 && !banco && lista.length === 0)) return null;

  const fazer = (p: Promise<void>) => {
    setOcupado(true);
    setErro(null);
    p.catch((e: unknown) => setErro(mensagemDe(e))).finally(() => setOcupado(false));
  };
  const ligar = (conexao: string | null) =>
    fazer(api.ligarBancoAnalise(projeto, analise, conexao));
  const decidir = (numero: number, d: DecisaoConsulta) =>
    fazer(api.decidirConsulta(projeto, analise, numero, d));
  const aguardando = pendentes(lista);

  return (
    <section className="painel-banco-analise" aria-label="Banco de dados">
      {erro && <Alerta tom="erro" titulo={erro} />}
      {!aprovada && bancos.length > 0 && (
        <fieldset className="banco-ligado" disabled={ocupado}>
          <legend>Banco de dados</legend>
          {bancos.map((b) => (
            <label key={b.id} className="check">
              <input
                type="checkbox"
                checked={banco?.conexao === b.id}
                disabled={!b.cadastrada}
                onChange={(e) => ligar(e.target.checked ? b.id : null)}
              />{" "}
              Usar o banco {b.nome} nesta análise
              {b.ambiente && <span className="muted"> ({b.ambiente})</span>}
            </label>
          ))}
          <p className="muted dica">
            Ligado, o agente pode consultar o banco só de leitura. Cada consulta espera a sua
            aprovação, e os dados pessoais chegam a ele sempre mascarados.
          </p>
        </fieldset>
      )}
      {aguardando.map((c) => (
        <PedidoConsulta
          key={c.numero}
          consulta={c}
          ocupado={ocupado || aprovada}
          aoDecidir={(d) => decidir(c.numero, d)}
        />
      ))}
      {lista.length > 0 && <RegistroConsultas consultas={lista} />}
    </section>
  );
}

/** Uma consulta aguardando o analista. */
function PedidoConsulta({
  consulta,
  ocupado,
  aoDecidir,
}: {
  consulta: ConsultaDados;
  ocupado: boolean;
  aoDecidir: (d: DecisaoConsulta) => void;
}) {
  const [modo, setModo] = useState<"ver" | "editar" | "recusar">("ver");
  const [sql, setSql] = useState(consulta.sql_proposto);
  const [motivo, setMotivo] = useState("");
  const idSql = useId();
  const idMotivo = useId();
  return (
    <div className="pedido-consulta" role="group" aria-label={`Consulta ${consulta.numero}`}>
      <div className="pedido-consulta-titulo">
        <strong>Consulta {consulta.numero} ao banco</strong>{" "}
        <Selo tom="atencao">aguardando sua aprovação</Selo>
      </div>
      <p>
        <span className="muted">Objetivo do agente:</span> {consulta.objetivo}
      </p>
      <p className="muted">
        {descreverOrigem(consulta.origem)} · pedida em {quando(consulta.pedida_ms)}
      </p>
      {modo === "editar" ? (
        <label className="field" htmlFor={idSql}>
          SQL a executar
          <textarea
            id={idSql}
            className="sql-consulta"
            rows={Math.min(16, sql.split("\n").length + 2)}
            value={sql}
            onChange={(e) => setSql(e.target.value)}
          />
        </label>
      ) : (
        <pre className="sql-consulta">
          <code>{consulta.sql_proposto}</code>
        </pre>
      )}
      {modo === "recusar" && (
        <label className="field" htmlFor={idMotivo}>
          Motivo (volta para o agente)
          <input
            id={idMotivo}
            value={motivo}
            onChange={(e) => setMotivo(e.target.value)}
            placeholder="Ex.: traz linhas inteiras de clientes; conte por situação"
          />
        </label>
      )}
      <div className="pedido-consulta-acoes">
        {modo === "ver" && (
          <>
            <button
              className="btn btn-sm btn-primary"
              disabled={ocupado}
              onClick={() => aoDecidir({ decisao: "executar" })}
            >
              Executar
            </button>
            <button className="btn btn-sm" disabled={ocupado} onClick={() => setModo("editar")}>
              Editar e executar
            </button>
            <button className="btn btn-sm" disabled={ocupado} onClick={() => setModo("recusar")}>
              Recusar
            </button>
          </>
        )}
        {modo === "editar" && (
          <>
            <button
              className="btn btn-sm btn-primary"
              disabled={ocupado || !sql.trim()}
              onClick={() => aoDecidir({ decisao: "editar_e_executar", sql })}
            >
              Executar o SQL editado
            </button>
            <button className="btn btn-sm" onClick={() => setModo("ver")}>
              Cancelar
            </button>
          </>
        )}
        {modo === "recusar" && (
          <>
            <button
              className="btn btn-sm btn-primary"
              disabled={ocupado || !motivo.trim()}
              onClick={() => aoDecidir({ decisao: "recusar", motivo: motivo.trim() })}
            >
              Recusar consulta
            </button>
            <button className="btn btn-sm" onClick={() => setModo("ver")}>
              Cancelar
            </button>
          </>
        )}
      </div>
    </div>
  );
}

/** Todas as consultas da análise: quem decidiu, quando, o SQL e as linhas. */
function RegistroConsultas({ consultas }: { consultas: ConsultaDados[] }) {
  return (
    <details className="registro-consultas">
      <summary>Consultas ao banco nesta análise ({consultas.length})</summary>
      <ul>
        {consultas.map((c) => (
          <li key={c.numero}>
            <div>
              <strong>Consulta {c.numero}</strong> · {ROTULO_SITUACAO[c.situacao]}
              {c.editada && " com edição"}
              {c.decidida_por && ` por ${c.decidida_por}`}
              {c.decidida_ms ? ` em ${quando(c.decidida_ms)}` : ""}
              {c.linhas != null && ` · ${c.linhas} linha(s)${c.cortada ? " (cortada)" : ""}`}
            </div>
            <div className="muted">{c.objetivo}</div>
            {c.motivo && <div className="muted">Motivo: {c.motivo}</div>}
            {c.erro && <div className="attention-text">{c.erro}</div>}
            {c.mascaradas && c.mascaradas.length > 0 && (
              <div className="muted">Colunas mascaradas: {c.mascaradas.join(", ")}</div>
            )}
            <pre className="sql-consulta">
              <code>{c.sql_aprovado ?? c.sql_proposto}</code>
            </pre>
          </li>
        ))}
      </ul>
    </details>
  );
}
