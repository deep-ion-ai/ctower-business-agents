import { useId, useState } from "react";
import { useConexoes } from "./ConexoesContexto";
import { FormConexao } from "./FormConexao";
import { conexoesPara, descritoresCom, type Capacidade, type Conexao } from "./conexoes";

const NOVA = "__nova__";

/**
 * Escolha da conexão de um lugar do projeto (#437): o rastreador, um repositório de
 * código ou o destino da documentação. Lista as conexões com a capacidade certa (e,
 * com `tipo`, só as daquele tipo) e oferece “Nova conexão…” ali mesmo, com o
 * formulário montado a partir do descritor.
 *
 * Vazio (`""`) é o cadastro sem conexão, que segue com o token e o comando do próprio
 * formulário. Fora de um {@link ProvedorConexoes}, ou sem descritores, não mostra nada.
 */
export function SeletorConexao({
  capacidade,
  tipo,
  valor,
  aoMudar,
  rotulo = "Conexão",
  desabilitado,
}: {
  capacidade: Capacidade;
  tipo?: string;
  valor: string;
  /** Recebe o `id` e a conexão escolhida (`null` sem conexão). */
  aoMudar: (id: string, conexao: Conexao | null) => void;
  rotulo?: string;
  desabilitado?: boolean;
}) {
  const id = useId();
  const ctx = useConexoes();
  const [criando, setCriando] = useState(false);
  if (!ctx || ctx.descritores.length === 0) return null;
  const opcoes = conexoesPara(ctx.conexoes, ctx.descritores, capacidade, tipo);
  const tipos = tipo ? [tipo] : descritoresCom(ctx.descritores, capacidade).map((d) => d.tipo);
  const atual = ctx.conexoes.find((c) => c.conexao.id === valor);

  return (
    <div className="field seletor-conexao">
      <label htmlFor={id}>{rotulo}</label>
      <select
        id={id}
        value={criando ? NOVA : valor}
        disabled={desabilitado}
        onChange={(e) => {
          const v = e.target.value;
          if (v === NOVA) {
            setCriando(true);
            return;
          }
          setCriando(false);
          aoMudar(v, ctx.conexoes.find((c) => c.conexao.id === v)?.conexao ?? null);
        }}
      >
        <option value="">Sem conexão (token e comando deste cadastro)</option>
        {valor && !atual && <option value={valor}>{valor} (não cadastrada)</option>}
        {opcoes.map((c) => (
          <option key={c.conexao.id} value={c.conexao.id}>
            {c.conexao.nome} ({c.rotuloTipo}, {c.conexao.endereco})
          </option>
        ))}
        <option value={NOVA}>Nova conexão…</option>
      </select>
      {atual?.ultimoTeste && !atual.ultimoTeste.pronta && (
        <span className="dica">Último teste da conexão falhou: {atual.ultimoTeste.resumo}</span>
      )}
      {criando && (
        <FormConexao
          api={ctx.api}
          descritores={ctx.descritores}
          tipos={tipos}
          aoCancelar={() => setCriando(false)}
          aoSalvar={(c) => {
            setCriando(false);
            void ctx.recarregar().then(() => aoMudar(c.id, c));
          }}
        />
      )}
    </div>
  );
}
