import { useState } from "react";
import { mensagemDe } from "./analise";
import type { PropsConfigProjeto } from "./ConfigProjeto";
import type { AmbienteDetalhado } from "./configuracoes";
import { DialogoConfirmacao } from "./DialogoConfirmacao";

/** Há algo do app na configuração do Claude deste projeto (servidor MCP ou hooks). */
export function temPreparo(a: AmbienteDetalhado | null | undefined): boolean {
  if (!a) return false;
  const v = a.verificacao;
  if (!v) return a.pronto;
  return v.servidorMcp !== "ausente" || v.hooks.some((h) => h.situacao !== "ausente");
}

/**
 * Aba “Avançado” (#192): a zona de cuidado, com “Desfazer o preparo…” (só quando há
 * preparo) e “Descadastrar…”. As duas pedem confirmação num diálogo; o descadastro
 * diz o que some e o que continua, e só vale digitando o nome do projeto.
 */
export function AbaAvancado({
  api,
  cfg,
  projeto,
  estado,
  aoMudar,
  aoReverificarAmbiente,
}: PropsConfigProjeto) {
  const [dialogo, setDialogo] = useState<"desfazer" | "descadastrar" | null>(null);
  const [nome, setNome] = useState("");
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [feito, setFeito] = useState<string[] | null>(null);
  const preparo = temPreparo(estado.ambiente);

  const fechar = () => {
    if (ocupado) return;
    setDialogo(null);
    setNome("");
    setFalha(null);
  };

  const desfazer = () => {
    setOcupado(true);
    setFalha(null);
    api
      .desfazerAmbiente(projeto.id)
      .then((r) => {
        setFeito(r.avisos);
        setDialogo(null);
        void aoReverificarAmbiente(projeto.id);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const descadastrar = () => {
    setOcupado(true);
    setFalha(null);
    api
      .descadastrar(projeto.id)
      .then(() => {
        setOcupado(false);
        aoMudar();
      })
      .catch((e: unknown) => {
        setFalha(`Não foi possível descadastrar: ${mensagemDe(e)}. O cadastro continua.`);
        setOcupado(false);
      });
  };

  const confirmado = nome.trim() === projeto.nome.trim();

  return (
    <div className="aba-avancado">
      {feito && (
        <div className="ok-text" role="status">
          <p>
            Preparo desfeito: a configuração do Claude voltou a ser a de antes para este projeto.
          </p>
          {feito.map((a) => (
            <p key={a} className="attention-text">
              {a}
            </p>
          ))}
        </div>
      )}
      <section className="zona-cuidado" aria-label="Zona de cuidado">
        <h4>Zona de cuidado</h4>
        {preparo && (
          <div className="linha-config zona-cuidado-item">
            <div className="saude-texto">
              <strong>Desfazer o preparo do agente</strong>
              <span className="muted">
                Tira deste projeto as ferramentas e os avisos do app na configuração do Claude. Suas
                análises e a base não mudam; dá para preparar de novo depois.
              </span>
            </div>
            <button
              className="btn btn-sm btn-danger"
              type="button"
              onClick={() => setDialogo("desfazer")}
            >
              Desfazer o preparo…
            </button>
          </div>
        )}
        <div className="linha-config zona-cuidado-item">
          <div className="saude-texto">
            <strong>Descadastrar o projeto</strong>
            <span className="muted">
              O projeto some do app e o preparo do agente é desfeito. Os documentos na base e as
              pastas do código continuam no computador.
            </span>
          </div>
          <button
            className="btn btn-sm btn-danger"
            type="button"
            onClick={() => setDialogo("descadastrar")}
          >
            Descadastrar…
          </button>
        </div>
      </section>

      {dialogo === "desfazer" && (
        <DialogoConfirmacao
          titulo="Desfazer o preparo do agente?"
          aoCancelar={fechar}
          acoes={
            <>
              <button className="btn" type="button" autoFocus disabled={ocupado} onClick={fechar}>
                Cancelar
              </button>
              <button
                className="btn btn-danger"
                type="button"
                disabled={ocupado}
                onClick={desfazer}
              >
                Desfazer o preparo
              </button>
            </>
          }
        >
          <p>
            O app vai tirar da configuração do Claude o servidor MCP deste projeto, a proteção
            contra edição (se ligada) e, se nenhum outro projeto usar, os avisos automáticos
            (hooks). Sem eles, as conversas nesta pasta não enxergam o app.
          </p>
          {falha && (
            <p className="error-text" role="alert">
              {falha}
            </p>
          )}
        </DialogoConfirmacao>
      )}

      {dialogo === "descadastrar" && (
        <DialogoConfirmacao
          titulo={`Descadastrar “${projeto.nome}”?`}
          aoCancelar={fechar}
          acoes={
            <>
              <button className="btn" type="button" autoFocus disabled={ocupado} onClick={fechar}>
                Cancelar
              </button>
              <button
                className="btn btn-danger"
                type="button"
                disabled={!confirmado || ocupado}
                onClick={descadastrar}
              >
                Descadastrar o projeto
              </button>
            </>
          }
        >
          <ul className="o-que-muda">
            <li>O projeto, o inventário e o mapa do código deixam de existir no app.</li>
            <li>O preparo do agente neste projeto é desfeito (e a proteção, se ligada).</li>
            <li>
              <strong className="ok-text">Continuam:</strong> os documentos do projeto na base
              {cfg.base ? (
                <>
                  {" "}
                  (<span className="caminho">{`${cfg.base}/${projeto.id}`}</span>)
                </>
              ) : null}{" "}
              e as pastas do código, que não são tocadas.
            </li>
          </ul>
          <label className="field">
            Para confirmar, digite o nome do projeto ({projeto.nome})
            <input
              type="text"
              autoComplete="off"
              spellCheck={false}
              value={nome}
              onChange={(e) => setNome(e.target.value)}
            />
          </label>
          {falha && (
            <p className="error-text" role="alert">
              {falha}
            </p>
          )}
        </DialogoConfirmacao>
      )}
    </div>
  );
}
