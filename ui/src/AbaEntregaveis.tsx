import { useEffect, useRef, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, BarraAcoes, Secao, Selo } from "./componentes";
import type { PropsConfigProjeto } from "./ConfigProjeto";
import {
  CICLO,
  DESTINOS,
  GERACAO,
  GRUPOS,
  METODOLOGIAS,
  ajustarTipo,
  alternarTipo,
  aplicarMetodologia,
  assinatura,
  escolherPadrao,
  rotuloDaMetodologia,
  type Destino,
  type Entregaveis,
  type EntregaveisDoProjeto,
  type Metodologia,
  type TipoEntregavel,
} from "./entregaveis";

/**
 * Aba “Entregáveis” do projeto (#460): a metodologia (Ágil, RUP, “Orientado a
 * agentes” ainda desabilitado, ou Personalizado) e o catálogo por grupo. Escolher a
 * metodologia aplica a predefinição dela; habilitar, desabilitar ou trocar o tipo
 * padrão à mão passa para Personalizado, com um aviso. Cada tipo habilitado tem o
 * template (o padrão do app ou um de `_templates/`) e o destino. Grava com “Salvar”;
 * sair da aba com alterações pede confirmação.
 */
export function AbaEntregaveis({ api, projeto, aoPendencia }: PropsConfigProjeto) {
  const [dados, setDados] = useState<EntregaveisDoProjeto | null | undefined>(undefined);
  const [config, setConfig] = useState<Entregaveis | null>(null);
  const [salvo, setSalvo] = useState("");
  const [passou, setPassou] = useState<Metodologia | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [gravado, setGravado] = useState(false);
  const pendencia = useRef(aoPendencia);
  pendencia.current = aoPendencia;

  useEffect(() => {
    let vivo = true;
    if (!api.entregaveisDoProjeto) {
      setDados(null);
      return;
    }
    api
      .entregaveisDoProjeto(projeto.id)
      .then((d) => {
        if (!vivo) return;
        setDados(d);
        setConfig(d.entregaveis);
        setSalvo(assinatura(d.entregaveis));
      })
      .catch((e: unknown) => {
        if (!vivo) return;
        setDados(null);
        setFalha(`Não foi possível ler os entregáveis: ${mensagemDe(e)}`);
      });
    return () => {
      vivo = false;
    };
  }, [api, projeto.id]);

  const pendente = config !== null && assinatura(config) !== salvo;
  useEffect(() => {
    pendencia.current?.(pendente);
  }, [pendente]);

  if (dados === undefined) return <p className="muted">Lendo os entregáveis…</p>;
  if (dados === null || config === null) {
    return <Alerta tom="erro" titulo={falha ?? "Os entregáveis não estão disponíveis."} />;
  }

  /** Mudança à mão: se tirou de Ágil ou RUP, avisa. */
  const mudar = (nova: Entregaveis) => {
    if (config.metodologia !== "personalizado" && nova.metodologia === "personalizado") {
      setPassou(config.metodologia);
    }
    setGravado(false);
    setConfig(nova);
  };

  const escolherMetodologia = (m: Metodologia) => {
    setPassou(null);
    setGravado(false);
    setConfig(aplicarMetodologia(dados, config, m));
  };

  const salvar = () => {
    const gravar = api.salvarEntregaveis;
    if (!gravar) return;
    setOcupado(true);
    setFalha(null);
    gravar(projeto.id, config)
      .then((e) => {
        setConfig(e);
        setSalvo(assinatura(e));
        setDados({ ...dados, entregaveis: e, configurada: true });
        setGravado(true);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const descartar = () => {
    setConfig(dados.entregaveis);
    setPassou(null);
    setFalha(null);
  };

  const habilitados = config.tipos.length;
  return (
    <form
      className="aba-entregaveis"
      onSubmit={(e) => {
        e.preventDefault();
        salvar();
      }}
    >
      <Secao titulo="Metodologia" nivel={4}>
        <fieldset className="linha-config metodologias">
          <legend className="sr-only">Metodologia do projeto</legend>
          {METODOLOGIAS.map((m) => (
            <label key={m.id} className="check">
              <input
                type="radio"
                name={`metodologia-${projeto.id}`}
                value={m.id}
                checked={config.metodologia === m.id}
                disabled={m.emBreve || ocupado}
                onChange={() => escolherMetodologia(m.id)}
              />{" "}
              {m.rotulo}
              {m.emBreve && (
                <>
                  {" "}
                  <Selo>em breve</Selo>
                </>
              )}
            </label>
          ))}
        </fieldset>
        <p className="muted">
          Escolher Ágil ou RUP habilita os tipos da predefinição. Habilitar, desabilitar ou trocar
          o tipo padrão à mão passa para Personalizado.
        </p>
        {!dados.configurada && !pendente && (
          <p className="muted">
            Este projeto ainda usa os tipos que o app já gerava: histórias, casos de teste,
            protótipo, manual e documento negocial.
          </p>
        )}
        {passou && config.metodologia === "personalizado" && (
          <Alerta tom="atencao" titulo="A metodologia passou para Personalizado.">
            Você mudou um item da predefinição de {rotuloDaMetodologia(passou)}. Para voltar a ela,
            escolha {rotuloDaMetodologia(passou)} de novo.
          </Alerta>
        )}
      </Secao>

      {GRUPOS.map((g) => (
        <Secao key={g.id} titulo={g.rotulo} nivel={4} className="grupo-entregaveis">
          <ul className="tipos-entregaveis">
            {dados.catalogo
              .filter((t) => t.grupo === g.id)
              .map((t) => (
                <ItemTipo
                  key={t.id}
                  tipo={t}
                  config={config}
                  templates={dados.templatesDaBase}
                  ultimo={habilitados === 1}
                  ocupado={ocupado}
                  aoMudar={mudar}
                  aoEscolherPadrao={(id) => mudar(escolherPadrao(dados.predefinicoes, config, id))}
                  aoAlternar={(id, sim) => mudar(alternarTipo(dados.catalogo, config, id, sim))}
                />
              ))}
          </ul>
        </Secao>
      ))}

      {falha && <Alerta tom="erro" titulo={falha} />}
      {gravado && !pendente && (
        <p className="ok-text" role="status">
          Entregáveis salvos ({rotuloDaMetodologia(config.metodologia)}).
        </p>
      )}
      <BarraAcoes rotulo="Ações dos entregáveis">
        <button
          className="btn btn-primary"
          type="submit"
          disabled={ocupado || !pendente || !api.salvarEntregaveis}
        >
          {ocupado ? "Salvando…" : "Salvar"}
        </button>
        {pendente && (
          <button className="btn" type="button" disabled={ocupado} onClick={descartar}>
            Desfazer as alterações
          </button>
        )}
      </BarraAcoes>
    </form>
  );
}

function ItemTipo({
  tipo: t,
  config,
  templates,
  ultimo,
  ocupado,
  aoMudar,
  aoEscolherPadrao,
  aoAlternar,
}: {
  tipo: TipoEntregavel;
  config: Entregaveis;
  templates: string[];
  /** Só um tipo habilitado: não dá para desabilitá-lo. */
  ultimo: boolean;
  ocupado: boolean;
  aoMudar: (e: Entregaveis) => void;
  aoEscolherPadrao: (id: string) => void;
  aoAlternar: (id: string, habilitar: boolean) => void;
}) {
  const habilitado = config.tipos.find((h) => h.tipo === t.id);
  const idBase = `entregavel-${t.id}`;
  return (
    <li className="tipo-entregavel" data-habilitado={habilitado ? "sim" : "nao"}>
      <label className="check">
        <input
          type="checkbox"
          checked={!!habilitado}
          disabled={ocupado || (!!habilitado && ultimo)}
          onChange={(e) => aoAlternar(t.id, e.target.checked)}
        />{" "}
        <strong>{t.nome}</strong>
      </label>{" "}
      <Selo>{CICLO[t.ciclo]}</Selo> <Selo tom={t.geracao === "agente" ? "agente" : "neutro"}>{GERACAO[t.geracao]}</Selo>
      {!t.disponivel && (
        <>
          {" "}
          <Selo tom="atencao">geração em breve</Selo>
        </>
      )}
      <p className="muted">{t.descricao}</p>
      {habilitado && (
        <div className="ajustes-entregavel">
          <label className="check">
            <input
              type="radio"
              name="tipo-padrao"
              checked={config.padrao === t.id}
              disabled={ocupado}
              onChange={() => aoEscolherPadrao(t.id)}
            />{" "}
            Tipo padrão
          </label>
          <label htmlFor={`${idBase}-template`}>
            Template de {t.nome}
            <select
              id={`${idBase}-template`}
              value={habilitado.template ?? ""}
              disabled={ocupado}
              onChange={(e) =>
                aoMudar(ajustarTipo(config, t.id, { template: e.target.value || null }))
              }
            >
              <option value="">Padrão do app ({t.template}.md)</option>
              {templates.map((a) => (
                <option key={a} value={a}>
                  _templates/{a}
                </option>
              ))}
            </select>
          </label>
          <label htmlFor={`${idBase}-destino`}>
            Destino de {t.nome}
            <select
              id={`${idBase}-destino`}
              value={habilitado.destino}
              disabled={ocupado}
              onChange={(e) =>
                aoMudar(ajustarTipo(config, t.id, { destino: e.target.value as Destino }))
              }
            >
              {DESTINOS.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.rotulo}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}
    </li>
  );
}
