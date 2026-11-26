import { useId, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta } from "./componentes";
import {
  conexaoNova,
  fonteVazia,
  type ApiConexoes,
  type CampoOpcao,
  type Conexao,
  type Descritor,
  type TipoFonte,
} from "./conexoes";

/**
 * Formulário de uma conexão (#437), montado a partir do descritor do tipo: o endereço
 * com o formato do tipo, as fontes de credencial que ele aceita e os campos das opções.
 * A credencial guardada no chaveiro é só digitada: vai direto para o chaveiro e nunca
 * volta para a tela.
 *
 * `tipos` limita os tipos oferecidos (o seletor de um rastreador Jira só cria Jira).
 */
export function FormConexao({
  api,
  descritores,
  inicial,
  tipos,
  credencialGuardada = false,
  aoSalvar,
  aoCancelar,
}: {
  api: ApiConexoes;
  descritores: Descritor[];
  /** A conexão em edição; sem ela, uma nova. */
  inicial?: Conexao;
  tipos?: string[];
  credencialGuardada?: boolean;
  aoSalvar: (c: Conexao) => void;
  aoCancelar: () => void;
}) {
  const id = useId();
  const oferecidos = descritores.filter((d) => !tipos || tipos.includes(d.tipo));
  const [valor, setValor] = useState<Conexao>(
    () => inicial ?? conexaoNova(oferecidos[0] ?? descritores[0]!),
  );
  const [segredo, setSegredo] = useState("");
  const [salvando, setSalvando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const d = descritores.find((x) => x.tipo === valor.tipo);
  const editando = Boolean(inicial?.id);

  const trocarTipo = (tipo: string) => {
    const novo = descritores.find((x) => x.tipo === tipo);
    if (novo) setValor({ ...conexaoNova(novo), nome: valor.nome });
  };
  const trocarFonte = (fonte: TipoFonte) => setValor({ ...valor, credencial: fonteVazia(fonte) });
  const opcao = (chave: string, v: string) =>
    setValor({ ...valor, opcoes: { ...(valor.opcoes ?? {}), [chave]: v } });

  const salvar = async () => {
    setSalvando(true);
    setFalha(null);
    try {
      const gravada = await api.salvarConexao(valor);
      if (gravada.credencial.fonte === "chaveiro" && segredo.trim()) {
        await api.guardarCredencialConexao(gravada.id, segredo);
      }
      setSegredo("");
      aoSalvar(gravada);
    } catch (erro) {
      setFalha(mensagemDe(erro));
    } finally {
      setSalvando(false);
    }
  };

  if (!d) return <Alerta tom="erro" titulo={`Tipo de conexão desconhecido: ${valor.tipo}`} />;
  const c = valor.credencial;
  return (
    // Um grupo, não um <form>: o formulário pode aparecer dentro do cadastro do projeto.
    <div
      role="group"
      className="form-conexao"
      aria-label={editando ? "Editar conexão" : "Nova conexão"}
    >
      <div className="field-pair">
        <div className="field">
          <label htmlFor={`${id}-tipo`}>Tipo</label>
          <select
            id={`${id}-tipo`}
            value={valor.tipo}
            disabled={editando}
            onChange={(e) => trocarTipo(e.target.value)}
          >
            {oferecidos.map((x) => (
              <option key={x.tipo} value={x.tipo}>
                {x.rotulo}
              </option>
            ))}
          </select>
        </div>
        <div className="field">
          <label htmlFor={`${id}-nome`}>Nome</label>
          <input
            id={`${id}-nome`}
            value={valor.nome}
            placeholder={`${d.rotulo} da empresa`}
            onChange={(e) => setValor({ ...valor, nome: e.target.value })}
          />
        </div>
      </div>
      <div className="field">
        <label htmlFor={`${id}-endereco`}>Endereço</label>
        <input
          id={`${id}-endereco`}
          className="caminho"
          spellCheck={false}
          value={valor.endereco}
          placeholder={d.endereco_padrao || d.formato_endereco}
          aria-describedby={`${id}-dica-endereco`}
          onChange={(e) => setValor({ ...valor, endereco: e.target.value })}
        />
        <span id={`${id}-dica-endereco`} className="dica">
          {d.formato_endereco}
        </span>
      </div>
      <div className="field">
        <label htmlFor={`${id}-fonte`}>Credencial</label>
        <select id={`${id}-fonte`} value={c.fonte} onChange={(e) => trocarFonte(e.target.value as TipoFonte)}>
          {d.fontes_aceitas.map((f) => (
            <option key={f.fonte} value={f.fonte}>
              {f.rotulo}
            </option>
          ))}
        </select>
      </div>
      {c.fonte === "chaveiro" && (
        <div className="field">
          <label htmlFor={`${id}-segredo`}>Token ou senha</label>
          <input
            id={`${id}-segredo`}
            type="password"
            className="caminho"
            autoComplete="off"
            spellCheck={false}
            value={segredo}
            placeholder={credencialGuardada ? "guardado; digite para trocar" : "cole o token"}
            onChange={(e) => setSegredo(e.target.value)}
          />
          <span className="dica">Fica no chaveiro do sistema, nunca no cadastro.</span>
        </div>
      )}
      {c.fonte === "comando" && (
        <div className="field">
          <label htmlFor={`${id}-cmd`}>Comando que imprime a credencial</label>
          <input
            id={`${id}-cmd`}
            className="caminho"
            spellCheck={false}
            value={c.cmd}
            placeholder="pass show servico | head -1"
            onChange={(e) => setValor({ ...valor, credencial: { fonte: "comando", cmd: e.target.value } })}
          />
        </div>
      )}
      {c.fonte === "arquivo" && (
        <div className="field">
          <label htmlFor={`${id}-arquivo`}>Arquivo da credencial</label>
          <input
            id={`${id}-arquivo`}
            className="caminho"
            spellCheck={false}
            value={c.caminho}
            onChange={(e) =>
              setValor({ ...valor, credencial: { fonte: "arquivo", caminho: e.target.value } })
            }
          />
        </div>
      )}
      {d.campos_opcoes.map((campo) => (
        <CampoDaOpcao
          key={campo.chave}
          campo={campo}
          valor={valor.opcoes?.[campo.chave] ?? ""}
          aoMudar={(v) => opcao(campo.chave, v)}
        />
      ))}
      {falha && <Alerta tom="erro" titulo={falha} />}
      <div className="acoes">
        <button
          type="button"
          className="btn btn-primary"
          disabled={salvando}
          onClick={() => void salvar()}
        >
          {editando ? "Salvar conexão" : "Criar conexão"}
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </div>
  );
}

/** Um campo de `opcoes`, pelo tipo que o descritor declara. */
function CampoDaOpcao({
  campo,
  valor,
  aoMudar,
}: {
  campo: CampoOpcao;
  valor: string;
  aoMudar: (v: string) => void;
}) {
  const id = useId();
  const ajuda = campo.ajuda ? (
    <span id={`${id}-ajuda`} className="dica">
      {campo.ajuda}
    </span>
  ) : null;
  if (campo.tipo === "booleano") {
    return (
      <label className="linha-checkbox">
        <input
          type="checkbox"
          checked={(valor || campo.padrao) === "true"}
          onChange={(e) => aoMudar(e.target.checked ? "true" : "false")}
        />{" "}
        {campo.rotulo}
      </label>
    );
  }
  return (
    <div className="field">
      <label htmlFor={id}>
        {campo.rotulo}
        {campo.obrigatorio ? "" : " (opcional)"}
      </label>
      {campo.tipo === "escolha" ? (
        <select id={id} value={valor || campo.padrao || ""} onChange={(e) => aoMudar(e.target.value)}>
          {!campo.padrao && <option value="">—</option>}
          {campo.opcoes.map((o) => (
            <option key={o.valor} value={o.valor}>
              {o.rotulo}
            </option>
          ))}
        </select>
      ) : (
        <input
          id={id}
          type={campo.tipo === "numero" ? "number" : "text"}
          min={campo.tipo === "numero" ? campo.min : undefined}
          max={campo.tipo === "numero" ? campo.max : undefined}
          value={valor}
          placeholder={campo.padrao ?? undefined}
          aria-describedby={ajuda ? `${id}-ajuda` : undefined}
          onChange={(e) => aoMudar(e.target.value)}
        />
      )}
      {ajuda}
    </div>
  );
}
