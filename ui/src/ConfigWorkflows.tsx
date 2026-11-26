import { useCallback, useEffect, useState } from "react";
import { mensagemDe } from "./analise";
import {
  Alerta,
  BarraAcoes,
  Cartao,
  EstadoVazio,
  ItemLista,
  ListaItens,
  Secao,
  Selo,
  type TomSelo,
} from "./componentes";
import { DialogoConfirmacao } from "./DialogoConfirmacao";
import type { PainelWorkflow } from "./EditorWorkflow";
import {
  DESTINO,
  ORIGEM,
  recadoDaMudanca,
  type ApiWorkflows,
  type ArquivoInstalado,
  type Copia,
  type Destino,
  type WorkflowVista,
} from "./workflows";

/** Abre o editor: a cópia de `destino`, ou o padrão (`null`), só leitura. */
export type AbrirWorkflow = (tipo: string, destino: Destino | null, painel?: PainelWorkflow) => void;

const EXPLICACAO: Record<Destino, string> = {
  projeto: "Fica na base (_templates/workflows/) e vale para os projetos dela; vai junto com a base no git.",
  global: "Fica neste computador e vale para todos os projetos, em qualquer base.",
};

/**
 * Configurações › Workflows (#372): por tipo de análise, a versão que as análises
 * usam (cópia na base > cópia para todos os projetos > padrão do app), as ações sobre
 * cada cópia (duplicar, editar, ver a diferença, restaurar o padrão), o aviso de que
 * o padrão do app mudou desde a cópia e, em cada projeto, o que está instalado. Um
 * workflow instalado editado à mão não é sobrescrito: a tela oferece levar a edição
 * para uma cópia.
 */
export function ConfigWorkflows({
  api,
  temBase,
  recarga,
  aoEditar,
}: {
  api: ApiWorkflows;
  temBase: boolean;
  /** Muda quando o editor fecha ou salva: ler de novo. */
  recarga: number;
  aoEditar: AbrirWorkflow;
}) {
  const [lista, setLista] = useState<WorkflowVista[] | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [recado, setRecado] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [restaurando, setRestaurando] = useState<{ w: WorkflowVista; destino: Destino } | null>(null);
  const [levando, setLevando] = useState<{
    w: WorkflowVista;
    projeto: string;
    arquivo: string;
    destino: Destino;
  } | null>(null);

  const carregar = useCallback(() => {
    api
      .workflows()
      .then((l) => {
        setLista(l);
        setFalha(null);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api]);

  useEffect(carregar, [carregar, recarga]);

  const executar = (acao: () => Promise<string>) => {
    setOcupado(true);
    setFalha(null);
    setRecado(null);
    acao()
      .then((r) => {
        setRecado(r);
        carregar();
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => {
        setOcupado(false);
        setRestaurando(null);
        setLevando(null);
      });
  };

  const duplicar = (w: WorkflowVista, destino: Destino) =>
    executar(async () => {
      const m = await api.duplicar(w.tipo, destino);
      aoEditar(w.tipo, destino);
      return recadoDaMudanca(m, `${DESTINO[destino]} de ${w.nome} criada.`);
    });

  const restaurar = (w: WorkflowVista, destino: Destino) =>
    executar(async () => {
      const m = await api.restaurar(w.tipo, destino);
      return recadoDaMudanca(m, `${DESTINO[destino]} de ${w.nome} apagada.`);
    });

  const levar = (w: WorkflowVista, projeto: string, arquivo: string, destino: Destino) =>
    executar(async () => {
      const m = await api.levarEdicao(projeto, w.tipo, arquivo, destino);
      const erros = m.leitura.validacao.problemas.some((p) => p.erro);
      return recadoDaMudanca(
        m,
        erros
          ? `A edição foi para a ${DESTINO[destino].toLowerCase()}, mas tem erros e ainda não é usada. Abra a cópia para corrigir.`
          : `A edição foi para a ${DESTINO[destino].toLowerCase()} e é a versão em uso.`,
      );
    });

  return (
    <Secao titulo="Workflows das análises">
      <p className="muted">
        Na condução aprofundada, o agente roda um workflow do Claude Code: um script com as fases da
        análise. O app traz um padrão para cada tipo. Você pode duplicá-lo e ajustar a cópia; a cópia
        na base vale antes da cópia para todos os projetos, e as duas antes do padrão. Atualizar o
        app nunca muda as suas cópias.
      </p>

      {falha && <Alerta tom="erro" titulo={falha} />}
      {recado && (
        <p className="muted" role="status">
          {recado}
        </p>
      )}
      {lista === null && !falha && <p className="muted">Carregando os workflows…</p>}

      {lista?.map((w) => (
        <Cartao key={w.tipo} rotulo={w.rotulo}>
          <div className="config-cartao-topo">
            <h3>
              {w.rotulo} <code>{w.nome}</code>
            </h3>
            <Selo tom={w.origemEfetiva === "padrao" ? "neutro" : "andamento"}>
              Em uso: {ORIGEM[w.origemEfetiva]}
            </Selo>
          </div>
          {w.fases.length > 0 && (
            <p className="muted fases-resumo">Fases: {w.fases.map((f) => f.titulo).join(" → ")}</p>
          )}
          <ListaItens>
            <ItemLista
              acoes={
                <button
                  className="btn btn-sm"
                  type="button"
                  aria-label={`Ver o padrão de ${w.nome}`}
                  onClick={() => aoEditar(w.tipo, null)}
                >
                  Ver o padrão
                </button>
              }
            >
              <span className="config-item-texto">
                <span className="config-item-nome">Padrão do app</span>
                <span className="muted">Vem com o app e muda quando ele é atualizado.</span>
              </span>
            </ItemLista>
            <LinhaCopia
              w={w}
              destino="global"
              copia={w.global}
              ocupado={ocupado}
              aoDuplicar={() => duplicar(w, "global")}
              aoEditar={aoEditar}
              aoRestaurar={() => setRestaurando({ w, destino: "global" })}
            />
            <LinhaCopia
              w={w}
              destino="projeto"
              copia={w.projeto}
              semBase={!temBase || w.projeto === null}
              ocupado={ocupado}
              aoDuplicar={() => duplicar(w, "projeto")}
              aoEditar={aoEditar}
              aoRestaurar={() => setRestaurando({ w, destino: "projeto" })}
            />
          </ListaItens>
          <div className="linha-config">
            <h4>Nos projetos</h4>
            {w.instalacoes.length === 0 ? (
              <EstadoVazio>Nenhum projeto cadastrado.</EstadoVazio>
            ) : (
              <ul className="instalacoes-workflow">
                {w.instalacoes.map((i) => (
                  <li key={i.projeto}>
                    <strong>{i.nome}</strong>
                    {i.erro ? (
                      <span className="muted"> — não foi possível conferir: {i.erro}</span>
                    ) : (
                      <ul>
                        {i.arquivos.map((a) => (
                          <li key={a.arquivo}>
                            <Selo tom={tomDaInstalacao(a)}>{SITUACAO[a.situacao]}</Selo>{" "}
                            {textoDaInstalacao(a)} <code className="caminho">{a.arquivo}</code>
                            {a.situacao === "divergente" && (
                              <span className="acoes">
                                {(["projeto", "global"] as const)
                                  .filter((d) => d === "global" || (temBase && w.projeto !== null))
                                  .map((d) => (
                                    <button
                                      key={d}
                                      type="button"
                                      className="btn btn-sm"
                                      disabled={ocupado}
                                      onClick={() =>
                                        setLevando({ w, projeto: i.projeto, arquivo: a.arquivo, destino: d })
                                      }
                                    >
                                      Levar a edição para a {DESTINO[d].toLowerCase()}
                                    </button>
                                  ))}
                              </span>
                            )}
                          </li>
                        ))}
                      </ul>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </div>
        </Cartao>
      ))}

      {restaurando && (
        <DialogoConfirmacao
          titulo={`Restaurar o padrão: ${restaurando.w.nome}`}
          aoCancelar={() => !ocupado && setRestaurando(null)}
          acoes={
            <>
              <button
                type="button"
                className="btn btn-primary"
                autoFocus
                disabled={ocupado}
                onClick={() => setRestaurando(null)}
              >
                Manter a cópia
              </button>
              <button
                type="button"
                className="btn btn-danger"
                disabled={ocupado}
                onClick={() => restaurar(restaurando.w, restaurando.destino)}
              >
                Apagar a cópia
              </button>
            </>
          }
        >
          <p>
            O app apaga a {DESTINO[restaurando.destino].toLowerCase()} e as próximas análises usam a
            versão de baixo. O que você mudou no script se perde.
          </p>
        </DialogoConfirmacao>
      )}
      {levando && (
        <DialogoConfirmacao
          titulo="Levar a edição para uma cópia"
          aoCancelar={() => !ocupado && setLevando(null)}
          acoes={
            <>
              <button
                type="button"
                className="btn btn-primary"
                autoFocus
                disabled={ocupado}
                onClick={() => setLevando(null)}
              >
                Cancelar
              </button>
              <button
                type="button"
                className="btn"
                disabled={ocupado}
                onClick={() => levar(levando.w, levando.projeto, levando.arquivo, levando.destino)}
              >
                Levar a edição
              </button>
            </>
          }
        >
          <p>
            O script editado em <code>{levando.arquivo}</code> vira a{" "}
            {DESTINO[levando.destino].toLowerCase()} de {levando.w.nome}, e o arquivo instalado volta
            a ser gerado pelo app.
            {copiaDe(levando.w, levando.destino)?.situacao !== "ausente" &&
              copiaDe(levando.w, levando.destino) !== null &&
              " A cópia que já existe é substituída."}
          </p>
        </DialogoConfirmacao>
      )}
    </Secao>
  );
}

function copiaDe(w: WorkflowVista, destino: Destino): Copia | null {
  return destino === "global" ? w.global : w.projeto;
}

function LinhaCopia({
  w,
  destino,
  copia,
  semBase = false,
  ocupado,
  aoDuplicar,
  aoEditar,
  aoRestaurar,
}: {
  w: WorkflowVista;
  destino: Destino;
  copia: Copia | null;
  semBase?: boolean;
  ocupado: boolean;
  aoDuplicar: () => void;
  aoEditar: AbrirWorkflow;
  aoRestaurar: () => void;
}) {
  const existe = copia !== null && copia.situacao !== "ausente";
  const rotulo = `${DESTINO[destino]} de ${w.nome}`;
  return (
    <ItemLista
      meta={<Selo tom={tomDaCopia(w, copia)}>{estadoDaCopia(w, copia)}</Selo>}
      acoes={
        !existe ? (
          <button
            className="btn btn-sm"
            type="button"
            disabled={ocupado || semBase}
            aria-label={`Duplicar ${w.nome}: ${DESTINO[destino].toLowerCase()}`}
            onClick={aoDuplicar}
          >
            Duplicar
          </button>
        ) : (
          <BarraAcoes rotulo={`Ações da ${rotulo.charAt(0).toLowerCase()}${rotulo.slice(1)}`} alinhamento="fim">
            <button
              className="btn btn-sm"
              type="button"
              aria-label={`Editar a ${rotulo.charAt(0).toLowerCase()}${rotulo.slice(1)}`}
              onClick={() => aoEditar(w.tipo, destino)}
            >
              Editar
            </button>
            <button
              className="btn btn-sm"
              type="button"
              aria-label={`Ver a diferença da ${rotulo.charAt(0).toLowerCase()}${rotulo.slice(1)} com o padrão`}
              onClick={() => aoEditar(w.tipo, destino, "diferenca")}
            >
              Ver a diferença
            </button>
            <button
              className="btn btn-sm"
              type="button"
              disabled={ocupado}
              aria-label={`Restaurar o padrão no lugar da ${rotulo.charAt(0).toLowerCase()}${rotulo.slice(1)}`}
              onClick={aoRestaurar}
            >
              Restaurar o padrão
            </button>
          </BarraAcoes>
        )
      }
    >
      <span className="config-item-texto">
        <span className="config-item-nome">{DESTINO[destino]}</span>
        <span className="muted">{EXPLICACAO[destino]}</span>
        {copia?.situacao === "invalida" && (
          <span className="error-text">Com erros, não é usada: {copia.erros}</span>
        )}
        {copia?.situacao === "ilegivel" && (
          <span className="error-text">Ilegível, não é usada: {copia.erro}</span>
        )}
        {existe && copia.padraoMudou && (
          <span className="aviso-padrao-novo" role="status">
            O padrão do app mudou desde esta cópia. A cópia continua valendo como está (o app não
            mescla).{" "}
            <button
              type="button"
              className="link"
              onClick={() => aoEditar(w.tipo, destino, "diferenca")}
            >
              Ver a diferença
            </button>
          </span>
        )}
        {semBase && <span className="muted">Escolha a pasta da base para ter uma cópia nela.</span>}
      </span>
    </ItemLista>
  );
}

function estadoDaCopia(w: WorkflowVista, c: Copia | null): string {
  if (c === null || c.situacao === "ausente") return "sem cópia";
  if (c.situacao === "invalida") return "com erros";
  if (c.situacao === "ilegivel") return "ilegível";
  if (w.origemEfetiva === c.origem) return "em uso";
  return "substituída pela cópia na base";
}

function tomDaCopia(w: WorkflowVista, c: Copia | null): TomSelo {
  if (c === null || c.situacao === "ausente") return "neutro";
  if (c.situacao !== "valida") return "erro";
  if (c.padraoMudou) return "atencao";
  return w.origemEfetiva === c.origem ? "andamento" : "neutro";
}

const SITUACAO: Record<ArquivoInstalado["situacao"], string> = {
  ok: "instalado",
  ausente: "não instalado",
  desatualizado: "desatualizado",
  divergente: "editado à mão",
  conflito: "conflito",
};

function textoDaInstalacao(a: ArquivoInstalado): string {
  switch (a.situacao) {
    case "ok":
      return a.origem ? `com o ${ORIGEM[a.origem]}:` : "";
    case "ausente":
      return "Prepare o projeto de novo (Configurar projeto › Agente) para instalar:";
    case "desatualizado":
      return "Outra versão; o próximo preparo atualiza:";
    case "divergente":
      return "O arquivo instalado foi editado à mão, e o app não o sobrescreve:";
    case "conflito":
      return "Há um arquivo com o mesmo nome que não é do app:";
  }
}

function tomDaInstalacao(a: ArquivoInstalado): TomSelo {
  if (a.situacao === "ok") return "concluido";
  if (a.situacao === "conflito") return "erro";
  return "atencao";
}
