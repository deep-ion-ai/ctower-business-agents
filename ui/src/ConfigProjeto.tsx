import { useCallback, useEffect, useState } from "react";
import { mensagemDe, type SituacaoProtecao } from "./analise";
import { AbrirTexto } from "./AbrirTexto";
import {
  ESTADO_DO_TEXTO,
  type AmbienteDetalhado,
  type ApiConfiguracoes,
  type Configuracoes,
  type ProjetoConfig,
  type TextoEditavel,
} from "./configuracoes";
import { FormRepositorios } from "./FormRepositorios";
import { AmbienteDoProjeto } from "./PrepararAmbiente";
import { padraoDoCaminho, type PreviaProjeto } from "./primeiroUso";
import { ProntidaoRepositorios } from "./ProntidaoRepositorios";
import { validarRepositorios, type Repositorios } from "./repositorios";
import { ResumoDoInventario } from "./ResumoDoInventario";

/**
 * Projeto escolhido no menu (#191). Por enquanto é o conteúdo que o cartão do projeto
 * já tinha (pastas, exclusões, repositórios, ambiente do Claude, proteção, regras e
 * descadastro); a #192 reescreve esta página em abas.
 */
export function ConfigProjeto({
  api,
  cfg,
  projeto,
  textos,
  aoMudar,
  aoAbrirTexto,
  aoEditar,
  aoAmbiente,
}: {
  api: ApiConfiguracoes;
  cfg: Pick<Configuracoes, "configClaude">;
  projeto: ProjetoConfig;
  textos: TextoEditavel[];
  aoMudar: () => void;
  aoAbrirTexto: () => void;
  aoEditar: (arquivo: string) => void;
  /** O ambiente do Claude do projeto foi verificado: o menu atualiza as pendências. */
  aoAmbiente: (projeto: string, pronto: boolean) => void;
}) {
  const ambiente = useCallback((pronto: boolean) => aoAmbiente(projeto.id, pronto), [aoAmbiente, projeto.id]);
  return (
    <section className="config-secao" aria-label="Projeto">
      {cfg.configClaude && (
        <p className="muted">
          O ambiente do Claude fica em {cfg.configClaude.estado} (servidor MCP) e{" "}
          {cfg.configClaude.settings} (hooks e proteção).
        </p>
      )}
      <CartaoProjeto
        key={projeto.id}
        api={api}
        projeto={projeto}
        regras={textos.find((t) => t.grupo === "regrasDoProjeto" && t.id === projeto.id)}
        aoMudar={aoMudar}
        aoAbrirTexto={aoAbrirTexto}
        aoEditar={aoEditar}
        aoAmbiente={ambiente}
      />
    </section>
  );
}

function CartaoProjeto({
  api,
  projeto,
  regras,
  aoMudar,
  aoAbrirTexto,
  aoEditar,
  aoAmbiente,
}: {
  api: ApiConfiguracoes;
  projeto: ProjetoConfig;
  regras?: TextoEditavel;
  aoMudar: () => void;
  aoAbrirTexto: () => void;
  aoEditar: (arquivo: string) => void;
  aoAmbiente: (pronto: boolean) => void;
}) {
  // Muda depois de desfazer o ambiente, para a verificação rodar de novo.
  const [vez, setVez] = useState(0);
  return (
    <article className="cartao-projeto" aria-label={projeto.nome}>
      <h4>{projeto.nome}</h4>
      <ul className="muted">
        {projeto.pastas.map((p) => (
          <li key={p.caminho}>
            {p.caminho}
            {p.prefixo && ` (aparece como ${p.prefixo}/)`}
            {!p.existe && <strong className="attention-text"> · a pasta não existe mais</strong>}
          </li>
        ))}
      </ul>
      <p className="muted">
        Para mudar as pastas, descadastre o projeto e cadastre de novo (os documentos ficam na
        base).
      </p>
      <AjusteDeExclusoes api={api} projeto={projeto} aoSalvar={aoMudar} />
      <RepositoriosDoProjeto api={api} projeto={projeto} aoSalvar={aoMudar} />
      <div role="group" aria-label={`Ambiente do Claude de ${projeto.nome}`}>
        <h5>Ambiente do Claude</h5>
        <DetalheDoAmbiente key={`d${vez}`} api={api} projeto={projeto.id} />
        <AmbienteDoProjeto key={vez} api={api} projeto={projeto.id} aoMudar={aoAmbiente} />
        <DesfazerAmbiente api={api} projeto={projeto} aoDesfazer={() => setVez((v) => v + 1)} />
      </div>
      <Protecao key={`p${vez}`} api={api} projeto={projeto} />
      {regras && (
        <div role="group" aria-label={`Regras do projeto ${projeto.nome}`}>
          <h5>Regras do projeto</h5>
          <p className="muted">
            Instruções que o agente recebe em toda análise deste projeto ({regras.arquivo}):{" "}
            {ESTADO_DO_TEXTO[regras.estado]}.
          </p>
          <AbrirTexto api={api} texto={regras} aoAbrir={aoAbrirTexto} aoEditar={aoEditar} />
        </div>
      )}
      <Descadastrar api={api} projeto={projeto} aoDescadastrar={aoMudar} />
    </article>
  );
}

/**
 * Seção “Repositórios” do projeto: onde fica o código e onde ficam as issues. O tipo de
 * cada um (e, no GitLab, host e caminho) não muda depois do cadastro. Mostra também a
 * prontidão das integrações, que não bloqueia o uso do app.
 */
function RepositoriosDoProjeto({
  api,
  projeto,
  aoSalvar,
}: {
  api: ApiConfiguracoes;
  projeto: ProjetoConfig;
  aoSalvar: () => void;
}) {
  const [valor, setValor] = useState<Repositorios>(projeto.repositorios);
  const [salvando, setSalvando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [salvo, setSalvo] = useState(false);
  const [vez, setVez] = useState(0);
  const mudou = JSON.stringify(valor) !== JSON.stringify(projeto.repositorios);

  const salvar = () => {
    const invalido = validarRepositorios(valor);
    if (invalido) {
      setFalha(invalido);
      return;
    }
    setSalvando(true);
    setFalha(null);
    setSalvo(false);
    api
      .salvarRepositorios(projeto.id, valor)
      .then(() => {
        setSalvo(true);
        setVez((v) => v + 1);
        aoSalvar();
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setSalvando(false));
  };

  return (
    <div role="group" aria-label={`Repositórios de ${projeto.nome}`}>
      <FormRepositorios
        valor={valor}
        aoMudar={setValor}
        travado={projeto.repositorios}
        api={api}
      />
      <div className="acoes">
        <button
          className="btn btn-primary"
          type="button"
          disabled={!mudou || salvando}
          onClick={salvar}
        >
          Salvar repositórios
        </button>
        {salvo && <span className="muted"> Salvo.</span>}
      </div>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      <ProntidaoRepositorios key={vez} api={api} projeto={projeto.id} />
    </div>
  );
}

function linhas(texto: string): string[] {
  return texto
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
}

function AjusteDeExclusoes({
  api,
  projeto,
  aoSalvar,
}: {
  api: ApiConfiguracoes;
  projeto: ProjetoConfig;
  aoSalvar: () => void;
}) {
  const [exclusoes, setExclusoes] = useState(projeto.exclusoes.join("\n"));
  const [inclusoes, setInclusoes] = useState<string[]>(projeto.inclusoes);
  const [previa, setPrevia] = useState<PreviaProjeto | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [salvo, setSalvo] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  const ajuste = (inc = inclusoes) => ({
    projeto: projeto.id,
    exclusoes: linhas(exclusoes),
    inclusoes: inc,
  });

  const verPrevia = (inc = inclusoes) => {
    setOcupado(true);
    setFalha(null);
    setSalvo(false);
    api
      .previaExclusoes(ajuste(inc))
      .then(setPrevia)
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const salvar = () => {
    setOcupado(true);
    setFalha(null);
    api
      .salvarExclusoes(ajuste())
      .then((s) => {
        setPrevia(null);
        setSalvo(true);
        setFalha(null);
        aoSalvar();
        return s;
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const mudarInclusoes = (novas: string[]) => {
    setInclusoes(novas);
    verPrevia(novas);
  };

  return (
    <details className="ajuste-exclusoes">
      <summary>Exclusões: o que fica fora da análise</summary>
      <label>
        Excluir também (um padrão por linha, como no .gitignore)
        <textarea
          rows={3}
          value={exclusoes}
          spellCheck={false}
          onChange={(e) => {
            setExclusoes(e.target.value);
            setSalvo(false);
          }}
        />
      </label>
      {inclusoes.length > 0 && (
        <div role="group" aria-label="Reincluídos">
          <p>Reincluídos por você:</p>
          <ul>
            {inclusoes.map((x) => (
              <li key={x}>
                <code>{x}</code>{" "}
                <button
                  className="btn"
                  type="button"
                  aria-label={`Tirar a reinclusão de ${x}`}
                  onClick={() => mudarInclusoes(inclusoes.filter((y) => y !== x))}
                >
                  Desfazer
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
      {previa && (
        <ResumoDoInventario
          inventario={previa.inventario}
          ocupado={ocupado}
          aoReincluir={(c) => {
            const p = padraoDoCaminho(c);
            if (!inclusoes.includes(p)) mudarInclusoes([...inclusoes, p]);
          }}
        />
      )}
      {salvo && (
        <p className="ok-text" role="status">
          Exclusões salvas e projeto inventariado de novo.
        </p>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      <div className="acoes">
        <button className="btn" type="button" onClick={() => verPrevia()} disabled={ocupado}>
          Ver o que será analisado
        </button>
        <button type="button" className="btn btn-primary" onClick={salvar} disabled={ocupado}>
          Salvar as exclusões
        </button>
      </div>
    </details>
  );
}

const SITUACAO = {
  ok: "registrado",
  ausente: "ausente",
  desatualizado: "desatualizado",
  conflito: "conflito (há um servidor “cba” que não é do app)",
} as const;

function DetalheDoAmbiente({ api, projeto }: { api: ApiConfiguracoes; projeto: string }) {
  const [ambiente, setAmbiente] = useState<AmbienteDetalhado | null>(null);
  useEffect(() => {
    let ativo = true;
    api
      .verificarAmbiente(projeto)
      .then((a) => ativo && setAmbiente(a))
      .catch(() => ativo && setAmbiente(null));
    return () => {
      ativo = false;
    };
  }, [api, projeto]);
  const v = ambiente?.verificacao;
  if (!v) return null;
  const hooksOk = v.hooks.filter((h) => h.situacao === "ok").length;
  return (
    <ul className="detalhe-ambiente" aria-label="Situação do ambiente">
      <li>Servidor MCP “cba”: {SITUACAO[v.servidorMcp]}</li>
      <li>
        Hooks: {hooksOk} de {v.hooks.length} registrados
        {v.hooksDesativados && " (desativados nas configurações do Claude)"}
      </li>
    </ul>
  );
}

function DesfazerAmbiente({
  api,
  projeto,
  aoDesfazer,
}: {
  api: ApiConfiguracoes;
  projeto: ProjetoConfig;
  aoDesfazer: () => void;
}) {
  const [confirmando, setConfirmando] = useState(false);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [feito, setFeito] = useState<string[] | null>(null);

  const desfazer = () => {
    setOcupado(true);
    setFalha(null);
    api
      .desfazerAmbiente(projeto.id)
      .then((r) => {
        setConfirmando(false);
        setFeito(r.avisos);
        aoDesfazer();
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  return (
    <div className="desfazer-ambiente">
      {feito && (
        <div className="ok-text" role="status">
          <p>
            Ambiente desfeito: a configuração do Claude voltou a ser a de antes para este projeto.
          </p>
          {feito.map((a) => (
            <p key={a} className="attention-text">
              {a}
            </p>
          ))}
        </div>
      )}
      {!confirmando ? (
        <button
          className="btn"
          type="button"
          onClick={() => {
            setFeito(null);
            setConfirmando(true);
          }}
        >
          Desfazer o ambiente
        </button>
      ) : (
        <div
          className="confirmar-alteracao"
          role="region"
          aria-label="Confirmar desfazer o ambiente"
        >
          <p>
            O app vai tirar da configuração do Claude o servidor MCP deste projeto, a proteção
            contra edição (se ligada) e, se nenhum outro projeto usar, os hooks. Sem eles, as
            conversas nesta pasta não enxergam o ctower. Você pode preparar de novo depois.
          </p>
          <button type="button" className="btn btn-primary" onClick={desfazer} disabled={ocupado}>
            Desfazer
          </button>{" "}
          <button
            className="btn"
            type="button"
            onClick={() => setConfirmando(false)}
            disabled={ocupado}
          >
            Cancelar
          </button>
        </div>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </div>
  );
}

function Protecao({ api, projeto }: { api: ApiConfiguracoes; projeto: ProjetoConfig }) {
  const [situacao, setSituacao] = useState<SituacaoProtecao | null>(null);
  const [confirmando, setConfirmando] = useState(false);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  useEffect(() => {
    let ativo = true;
    api
      .situacaoProtecao(projeto.id)
      .then((s) => ativo && setSituacao(s))
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, projeto.id]);

  const mudar = (ligada: boolean) => {
    setOcupado(true);
    setFalha(null);
    api
      .mudarProtecao(projeto.id, ligada)
      .then((s) => {
        setSituacao(s);
        setConfirmando(false);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  return (
    <div role="group" aria-label={`Proteção contra edição de ${projeto.nome}`}>
      <h5>Proteção contra edição</h5>
      {situacao && (
        <p>
          {situacao.ligada
            ? situacao.completa
              ? "Ligada: o Claude não edita arquivos nem usa o terminal nas pastas do projeto."
              : "Ligada, mas faltam regras no arquivo de configuração do Claude. Ligue de novo para completar."
            : "Desligada (padrão): o app só avisa quando a pasta do projeto é alterada."}
        </p>
      )}
      {situacao && !confirmando && (
        <button className="btn" type="button" onClick={() => setConfirmando(true)}>
          {situacao.ligada ? "Desligar a proteção" : "Ligar a proteção"}
        </button>
      )}
      {situacao && confirmando && (
        <div className="confirmar-alteracao">
          {situacao.ligada ? (
            <p>
              O app vai tirar de {situacao.arquivo} só as regras que ele mesmo acrescentou, e o
              terminal (Bash) volta a funcionar nas conversas deste projeto.
            </p>
          ) : (
            <>
              <p>
                O app vai acrescentar em {situacao.arquivo} estas regras, que valem mesmo que você
                aprove a edição, e negar o terminal ({situacao.negadasPeloHook.join(", ")}) nas
                conversas deste projeto:
              </p>
              <ul>
                {situacao.regras.map((r) => (
                  <li key={r.regra}>
                    <code>{r.regra}</code>
                  </li>
                ))}
              </ul>
            </>
          )}
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => mudar(!situacao.ligada)}
            disabled={ocupado}
          >
            {situacao.ligada ? "Concordo, desligar" : "Concordo, ligar"}
          </button>{" "}
          <button
            className="btn"
            type="button"
            onClick={() => setConfirmando(false)}
            disabled={ocupado}
          >
            Cancelar
          </button>
        </div>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </div>
  );
}

function Descadastrar({
  api,
  projeto,
  aoDescadastrar,
}: {
  api: ApiConfiguracoes;
  projeto: ProjetoConfig;
  aoDescadastrar: () => void;
}) {
  const [confirmando, setConfirmando] = useState(false);
  const [confirmado, setConfirmado] = useState(false);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  const descadastrar = () => {
    setOcupado(true);
    setFalha(null);
    api
      .descadastrar(projeto.id)
      .then(() => aoDescadastrar())
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setOcupado(false);
      });
  };

  return (
    <div className="descadastrar">
      {!confirmando ? (
        <button type="button" className="link perigo" onClick={() => setConfirmando(true)}>
          Descadastrar o projeto
        </button>
      ) : (
        <div
          className="confirmar-alteracao"
          role="region"
          aria-label={`Confirmar o descadastro de ${projeto.nome}`}
        >
          <p>
            O app vai desfazer o ambiente do Claude deste projeto (servidor MCP, proteção contra
            edição e, se for o último projeto, os hooks) e apagar o cadastro, o inventário e o mapa.
            Os documentos do projeto na base de conhecimento ficam, e as pastas do código não são
            tocadas.
          </p>
          <label>
            <input
              type="checkbox"
              checked={confirmado}
              onChange={(e) => setConfirmado(e.target.checked)}
            />{" "}
            Confirmo o descadastro de {projeto.nome}
          </label>
          <div className="acoes">
            <button
              className="btn"
              type="button"
              onClick={() => setConfirmando(false)}
              disabled={ocupado}
            >
              Cancelar
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={descadastrar}
              disabled={!confirmado || ocupado}
            >
              Descadastrar
            </button>
          </div>
        </div>
      )}
      {falha && (
        <p className="error-text" role="alert">
          Não foi possível descadastrar: {falha}. O cadastro continua.
        </p>
      )}
    </div>
  );
}
