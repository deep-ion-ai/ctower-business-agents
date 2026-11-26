import { useEffect, useId, useState } from "react";
import type { Afirmacao, OrigemPergunta, Pergunta, TermoProposto } from "./analise";
import { DialogoConfirmacao } from "./DialogoConfirmacao";
import type { DecisaoRevisao } from "./revisao";
import { TextoAfirmacao } from "./TextoAfirmacao";
import { Alerta, BotaoIcone, Selo } from "./componentes";
import { Icon, type IconName } from "./shell/icons";

/** Botão de ação da revisão: só o ícone, com o rótulo no nome acessível e na dica (#130). */
function BotaoAcao({
  icone,
  rotulo,
  desabilitado,
  aoClicar,
}: {
  icone: IconName;
  rotulo: string;
  desabilitado?: boolean;
  aoClicar: () => void;
}) {
  return (
    <BotaoIcone rotulo={rotulo} disabled={desabilitado} onClick={aoClicar}>
      <Icon name={icone} />
    </BotaoIcone>
  );
}

/** Botão de ação com o rótulo à vista (#184): Confirmar, Incorreta, Aprofundar. */
function BotaoRotulado({
  icone,
  rotulo,
  nome,
  tom,
  desabilitado,
  aoClicar,
}: {
  icone: IconName;
  rotulo: string;
  /** Nome acessível, quando precisa dizer mais que o rótulo à vista. */
  nome?: string;
  tom?: "ok" | "erro";
  desabilitado?: boolean;
  aoClicar: () => void;
}) {
  return (
    <button
      className="btn btn-sm btn-acao-rotulada"
      type="button"
      aria-label={nome}
      title={nome ?? rotulo}
      data-tom={tom}
      disabled={desabilitado}
      onClick={aoClicar}
    >
      <Icon name={icone} size={14} />
      {rotulo}
    </button>
  );
}

/** Como o documento leva as decisões do analista à revisão (#37). */
export interface Revisao {
  /** Aplica a decisão; a promessa rejeita com a mensagem da recusa. */
  aoDecidir: (d: DecisaoRevisao) => Promise<void>;
  /** Uma decisão está em curso: os botões ficam desabilitados. */
  ocupado: boolean;
  /**
   * Envio das respostas ao agente. Sem ele, ou sem `disponivel` (a conversa não
   * está aberta aqui), o botão aparece desabilitado com a explicação.
   */
  respostas?: {
    disponivel: boolean;
    aoEnviar: (perguntas: Pergunta[]) => void;
  };
}

/** Respostas dadas pelo analista que o agente ainda não recebeu. */
export function respostasAEnviar(perguntas: Pergunta[]): Pergunta[] {
  return perguntas.filter((p) => p.estado === "respondida" && !p.resposta_enviada);
}

/**
 * "Enviar respostas": manda ao agente, de uma vez, as respostas ainda não enviadas.
 * Não precisa que todas as perguntas estejam respondidas.
 */
export function EnviarRespostas({
  perguntas,
  revisao,
}: {
  perguntas: Pergunta[];
  revisao: Revisao;
}) {
  const pendentes = respostasAEnviar(perguntas);
  const disponivel = !!revisao.respostas?.disponivel;
  if (pendentes.length === 0) return null;
  return (
    <div className="enviar-respostas">
      <button
        type="button"
        className="btn btn-primary"
        disabled={revisao.ocupado || !disponivel}
        onClick={() => revisao.respostas?.aoEnviar(pendentes)}
      >
        <Icon name="reply" />{" "}
        {pendentes.length === 1
          ? "Enviar resposta ao agente"
          : `Enviar ${pendentes.length} respostas ao agente`}
      </button>
      <span className="muted">
        {disponivel
          ? pendentes.length === 1
            ? "1 resposta ainda não foi enviada ao agente."
            : `${pendentes.length} respostas ainda não foram enviadas ao agente.`
          : "Abra a conversa aqui para enviar as respostas ao agente."}
      </span>
    </div>
  );
}

/**
 * Pedido de aprofundamento: o analista detalha o que quer ver (opcional). Fica
 * registrado na afirmação e segue no prompt ao agente.
 */
function DialogoAprofundar({
  afirmacao,
  ocupado,
  aoPedir,
  aoCancelar,
}: {
  afirmacao: Afirmacao;
  ocupado: boolean;
  aoPedir: (pedido: string) => void;
  aoCancelar: () => void;
}) {
  const [pedido, setPedido] = useState(
    afirmacao.estado === "aprofundar" ? (afirmacao.pedido ?? "") : "",
  );
  const id = useId();
  const formId = `${id}-form`;
  return (
    <DialogoConfirmacao
      titulo={`Aprofundar a afirmação ${afirmacao.numero}`}
      aoCancelar={aoCancelar}
      acoes={
        <>
          <button type="submit" form={formId} className="btn btn-primary" disabled={ocupado}>
            Pedir aprofundamento
          </button>
          <button type="button" className="btn" onClick={aoCancelar}>
            Cancelar
          </button>
        </>
      }
    >
      <form
        id={formId}
        className="dialogo-aprofundar"
        onSubmit={(e) => {
          e.preventDefault();
          aoPedir(pedido.trim());
        }}
      >
        <blockquote className="afirmacao">
          <TextoAfirmacao texto={afirmacao.texto} />
        </blockquote>
        <label htmlFor={id}>O que você quer que o agente aprofunde? (opcional)</label>
        <textarea
          id={id}
          value={pedido}
          rows={4}
          autoFocus
          placeholder="Ex.: quais perfis podem fazer essa consulta e onde isso é verificado"
          onChange={(e) => setPedido(e.target.value)}
        />
      </form>
    </DialogoConfirmacao>
  );
}

/** Pedido de ação numa afirmação, vindo dos atalhos do teclado. */
export interface ComandoAfirmacao {
  numero: number;
  tipo: "incorreta" | "aprofundar";
}

type Formulario = { tipo: "incorreta" } | { tipo: "editar" } | { tipo: "aprofundar" } | null;

/**
 * Ações do analista numa afirmação: confirmar (vira regra na aprovação), marcar
 * como incorreta (com comentário; fica registrada e não vira regra), pedir
 * aprofundamento, desfazer a decisão e editar o texto (as evidências ficam).
 */
export function AcoesAfirmacao({
  afirmacao,
  revisao,
  comando,
}: {
  afirmacao: Afirmacao;
  revisao: Revisao;
  /** Pedido vindo dos atalhos do teclado (X, A): abre o formulário da ação. */
  comando?: ComandoAfirmacao | null;
}) {
  const [form, setForm] = useState<Formulario>(null);
  const [texto, setTexto] = useState("");
  const id = useId();
  const { numero, estado } = afirmacao;
  const decidida = estado === "confirmada" || estado === "incorreta" || estado === "aprofundar";

  const decidir = (d: DecisaoRevisao) =>
    revisao
      .aoDecidir(d)
      .then(() => setForm(null))
      .catch(() => undefined);

  const abrir = (tipo: "incorreta" | "editar") => {
    if (tipo === "editar") setTexto(afirmacao.texto);
    else setTexto(afirmacao.estado === "incorreta" ? afirmacao.comentario : "");
    setForm({ tipo });
  };

  // Os atalhos X e A abrem o formulário da ação, como o clique no botão.
  useEffect(() => {
    if (!comando || comando.numero !== numero || revisao.ocupado) return;
    if (comando.tipo === "aprofundar") setForm({ tipo: "aprofundar" });
    else abrir("incorreta");
    // Só o pedido novo conta; o resto da afirmação vem do render atual.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [comando]);

  return (
    <div className="acoes-revisao" role="group" aria-label={`Revisão da afirmação ${numero}`}>
      {form?.tipo === "aprofundar" && (
        <DialogoAprofundar
          afirmacao={afirmacao}
          ocupado={revisao.ocupado}
          aoCancelar={() => setForm(null)}
          aoPedir={(pedido) =>
            decidir({ tipo: "afirmacao", numero, estado: "aprofundar", pedido: pedido || undefined })
          }
        />
      )}
      {(!form || form.tipo === "aprofundar") && (
        <>
          <BotaoRotulado
            icone="check"
            rotulo="Confirmar"
            tom="ok"
            desabilitado={revisao.ocupado || estado === "confirmada"}
            aoClicar={() => decidir({ tipo: "afirmacao", numero, estado: "confirmada" })}
          />
          <BotaoRotulado
            icone="flag"
            rotulo="Incorreta"
            tom="erro"
            desabilitado={revisao.ocupado}
            aoClicar={() => abrir("incorreta")}
          />
          <BotaoRotulado
            icone="deepen"
            rotulo="Aprofundar"
            nome={estado === "aprofundar" ? "Pedir o aprofundamento de novo" : undefined}
            desabilitado={revisao.ocupado}
            aoClicar={() => setForm({ tipo: "aprofundar" })}
          />
          <BotaoAcao
            icone="pencil"
            rotulo="Editar texto"
            desabilitado={revisao.ocupado}
            aoClicar={() => abrir("editar")}
          />
          {decidida && (
            <BotaoRotulado
              icone="undo"
              rotulo="Desfazer"
              nome="Desfazer decisão"
              desabilitado={revisao.ocupado}
              aoClicar={() => decidir({ tipo: "afirmacao", numero, estado: "registrada" })}
            />
          )}
        </>
      )}
      {form && form.tipo !== "aprofundar" && (
        <form
          className="form-revisao"
          onSubmit={(e) => {
            e.preventDefault();
            const valor = texto.trim();
            if (!valor) return;
            decidir(
              form.tipo === "incorreta"
                ? { tipo: "afirmacao", numero, estado: "incorreta", comentario: valor }
                : { tipo: "editar-afirmacao", numero, texto: valor },
            );
          }}
        >
          <label htmlFor={id}>
            {form.tipo === "incorreta"
              ? "Por que está incorreta? O comentário fica registrado na análise."
              : "Texto da afirmação (as evidências ficam como estão)"}
          </label>
          <textarea
            id={id}
            value={texto}
            rows={3}
            autoFocus
            onChange={(e) => setTexto(e.target.value)}
          />
          <div>
            <button className="btn" type="submit" disabled={revisao.ocupado || !texto.trim()}>
              {form.tipo === "incorreta" ? "Marcar como incorreta" : "Salvar texto"}
            </button>{" "}
            <button className="btn" type="button" onClick={() => setForm(null)}>
              Cancelar
            </button>
          </div>
        </form>
      )}
    </div>
  );
}

/**
 * Resposta no próprio cartão da pergunta (#184): o campo já vem à vista, com
 * "Salvar resposta" e "Descartar".
 */
export function AcoesPergunta({ pergunta, revisao }: { pergunta: Pergunta; revisao: Revisao }) {
  const [resposta, setResposta] = useState("");
  const id = useId();
  const { numero } = pergunta;
  if (pergunta.estado !== "aberta") return null;
  const decidir = (d: DecisaoRevisao) =>
    revisao
      .aoDecidir(d)
      .then(() => setResposta(""))
      .catch(() => undefined);
  return (
    <form
      className="form-revisao form-resposta"
      aria-label={`Revisão da pergunta ${numero}`}
      onSubmit={(e) => {
        e.preventDefault();
        if (resposta.trim())
          decidir({ tipo: "pergunta", numero, estado: "respondida", resposta: resposta.trim() });
      }}
    >
      <label htmlFor={id}>Resposta</label>
      <textarea
        id={id}
        value={resposta}
        rows={2}
        placeholder="Escreva a resposta do negócio"
        onChange={(e) => setResposta(e.target.value)}
      />
      <div className="acoes-revisao">
        <button
          className="btn btn-sm btn-acao-rotulada"
          type="submit"
          disabled={revisao.ocupado || !resposta.trim()}
        >
          <Icon name="check" size={14} />
          Salvar resposta
        </button>
        <button
          className="btn btn-sm btn-acao-rotulada"
          type="button"
          disabled={revisao.ocupado}
          onClick={() => decidir({ tipo: "pergunta", numero, estado: "descartada" })}
        >
          <Icon name="trash" size={14} />
          Descartar
        </button>
      </div>
    </form>
  );
}

/** Mesma origem de pergunta (o item já virou pergunta). */
export function mesmaOrigem(a: OrigemPergunta | null | undefined, b: OrigemPergunta): boolean {
  if (!a || a.diagrama !== b.diagrama) return false;
  if (a.tipo === "aresta" && b.tipo === "aresta") return a.de === b.de && a.para === b.para;
  if (a.tipo === "achado-diagrama" && b.tipo === "achado-diagrama") return a.achado === b.achado;
  return false;
}

/**
 * "Virar pergunta" (#120): o analista transforma um item da análise (a relação
 * inferida de um modelo de dados, por exemplo) numa pergunta em aberto, com o
 * texto já preenchido e editável; o app copia as evidências do item. O item que já
 * virou pergunta mostra o número dela, sem botão, para não duplicar.
 */
export function VirarPergunta({
  origem,
  textoInicial,
  perguntas,
  revisao,
}: {
  origem: OrigemPergunta;
  textoInicial: string;
  perguntas: Pergunta[];
  revisao?: Revisao;
}) {
  const [editando, setEditando] = useState(false);
  const [texto, setTexto] = useState(textoInicial);
  const [erro, setErro] = useState<string | null>(null);
  const id = useId();
  const ja = perguntas.find((p) => mesmaOrigem(p.origem, origem));
  if (ja) {
    return (
      <Selo tom="concluido" titulo={ja.texto}>
        virou a pergunta {ja.numero}
      </Selo>
    );
  }
  if (!revisao) return null;
  if (editando)
    return (
      <form
        className="form-revisao"
        onSubmit={(e) => {
          e.preventDefault();
          if (!texto.trim()) return;
          revisao
            .aoDecidir({ tipo: "virar-pergunta", texto: texto.trim(), origem })
            .then(() => setEditando(false))
            .catch((m: unknown) => setErro(String(m)));
        }}
      >
        <label htmlFor={id}>Pergunta</label>
        <textarea id={id} value={texto} rows={3} onChange={(e) => setTexto(e.target.value)} />
        {erro && <Alerta tom="erro" titulo={erro} />}
        <div>
          <button className="btn" type="submit" disabled={revisao.ocupado || !texto.trim()}>
            Registrar pergunta
          </button>{" "}
          <button className="btn" type="button" onClick={() => setEditando(false)}>
            Cancelar
          </button>
        </div>
      </form>
    );
  return (
    <button
      type="button"
      className="btn btn-sm"
      title="Registra uma pergunta em aberto com este item e a evidência dele, sem passar pelo agente"
      disabled={revisao.ocupado}
      onClick={() => {
        setTexto(textoInicial);
        setErro(null);
        setEditando(true);
      }}
    >
      Virar pergunta
    </button>
  );
}

/**
 * Termo proposto para o glossário: aceitar, recusar e editar a definição, que o
 * analista pode melhorar antes de ela ir para o glossário (#130).
 */
export function AcoesTermo({ termo, revisao }: { termo: TermoProposto; revisao: Revisao }) {
  const [editando, setEditando] = useState(false);
  const [definicao, setDefinicao] = useState("");
  const id = useId();
  const { numero, estado } = termo;
  if (estado === "recusado") return null;
  if (editando)
    return (
      <form
        className="form-revisao"
        onSubmit={(e) => {
          e.preventDefault();
          const valor = definicao.trim();
          if (!valor) return;
          revisao
            .aoDecidir({ tipo: "editar-termo", numero, definicao: valor })
            .then(() => setEditando(false))
            .catch(() => undefined);
        }}
      >
        <label htmlFor={id}>Definição de "{termo.termo}" para o glossário</label>
        <textarea
          id={id}
          value={definicao}
          rows={3}
          onChange={(e) => setDefinicao(e.target.value)}
        />
        <div>
          <button className="btn" type="submit" disabled={revisao.ocupado || !definicao.trim()}>
            Salvar definição
          </button>{" "}
          <button className="btn" type="button" onClick={() => setEditando(false)}>
            Cancelar
          </button>
        </div>
      </form>
    );
  const decidir = (aceitar: boolean) =>
    revisao
      .aoDecidir({ tipo: "termo", numero, estado: aceitar ? "aceito" : "recusado" })
      .catch(() => undefined);
  return (
    <div className="acoes-revisao" role="group" aria-label={`Revisão do termo ${termo.termo}`}>
      {estado === "proposto" && (
        <>
          <BotaoRotulado
            icone="check"
            rotulo="Aceitar"
            tom="ok"
            desabilitado={revisao.ocupado}
            aoClicar={() => decidir(true)}
          />
          <BotaoRotulado
            icone="close"
            rotulo="Recusar"
            tom="erro"
            desabilitado={revisao.ocupado}
            aoClicar={() => decidir(false)}
          />
        </>
      )}
      <BotaoAcao
        icone="pencil"
        rotulo="Editar definição"
        desabilitado={revisao.ocupado}
        aoClicar={() => {
          setDefinicao(termo.definicao);
          setEditando(true);
        }}
      />
    </div>
  );
}

/** Aceitar ou recusar uma sugestão. */
export function AcoesAceite({
  tipo,
  numero,
  rotulo,
  revisao,
}: {
  tipo: "sugestao" | "termo";
  numero: number;
  rotulo: string;
  revisao: Revisao;
}) {
  const decidir = (aceitar: boolean) => {
    const d: DecisaoRevisao =
      tipo === "sugestao"
        ? { tipo, numero, estado: aceitar ? "aceita" : "recusada" }
        : { tipo, numero, estado: aceitar ? "aceito" : "recusado" };
    revisao.aoDecidir(d).catch(() => undefined);
  };
  return (
    <div className="acoes-revisao" role="group" aria-label={`Revisão ${rotulo}`}>
      <BotaoRotulado
        icone="check"
        rotulo="Aceitar"
        tom="ok"
        desabilitado={revisao.ocupado}
        aoClicar={() => decidir(true)}
      />
      <BotaoRotulado
        icone="close"
        rotulo="Recusar"
        tom="erro"
        desabilitado={revisao.ocupado}
        aoClicar={() => decidir(false)}
      />
    </div>
  );
}
